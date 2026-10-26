import type { Progress, QueueItem, RateLimit } from './types.ts';

/**
 * How long the queue still needs, learned from the albums it has already finished.
 *
 * A queue mixes albums of ten tracks and albums of five hundred, and nothing says which is which
 * before it starts: the size of what waits is guessed from the size of what was done, and the
 * uncertainty of that guess is told as a range instead of hidden. The waits that are set (between
 * albums, long breaks) are counted for what they are, from the settings; the wait before each track
 * and the speed of the transfers are learned, since they are part of how long a track took.
 */

/** The width of the range: 80 % of the outcomes lie within it. */
const Z = 1.2816;
/** Fewer finished albums than this and any figure would be a guess about a guess. */
const MIN_SAMPLES = 3;
/** The albums looked at: the latest ones, which are those the current settings produced. */
const RECENT_TIMED = 40;
const RECENT_SIZED = 200;
/** An album that took this many times the usual time per track was interrupted (a pause, a sign-out). */
const OUTLIER = 4;

export interface Estimate {
	/** Seconds left, as expected, and the two ends of the range. */
	seconds: number;
	low: number;
	high: number;
	/** Items waiting whose size cannot be guessed (a discography, a playlist): they are not counted. */
	uncounted: number;
	/** What the figure rests on. */
	basis: {
		albums: number;
		secondsPerTrack: number;
		tracksPerAlbum: number;
		/** The long breaks that the expected downloads run into. */
		breaks: number;
	};
}

interface Input {
	queue: readonly QueueItem[];
	progress: Progress;
	limits: RateLimit;
}

const sum = (values: number[]) => values.reduce((total, value) => total + value, 0);
const mean = (values: number[]) => (values.length ? sum(values) / values.length : 0);

function spread(values: number[]): number {
	if (values.length < 2) return 0;
	const middle = mean(values);
	return Math.sqrt(sum(values.map((value) => (value - middle) ** 2)) / (values.length - 1));
}

function median(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	const half = Math.floor(sorted.length / 2);
	return sorted.length % 2 ? sorted[half] : (sorted[half - 1] + sorted[half]) / 2;
}

/** The latest albums of the queue that were completed, newest first. */
function lastAlbums(queue: readonly QueueItem[], keep: (item: QueueItem) => boolean, count: number) {
	const found: QueueItem[] = [];
	for (let at = queue.length - 1; at >= 0 && found.length < count; at--) {
		const item = queue[at];
		if (item.kind === 'album' && item.status === 'completed' && keep(item)) found.push(item);
	}
	return found;
}

interface Pace {
	/** Seconds to write a track, and how sure that is (relative). */
	perTrack: number;
	uncertainty: number;
	/** Megabytes a written track weighs, `0` when not known. */
	megabytes: number;
	samples: number;
}

/** How long a written track takes, from the albums that wrote some. */
function learnPace(queue: readonly QueueItem[]): Pace | null {
	const timed = lastAlbums(
		queue,
		(item) => item.downloaded > 0 && item.started_at !== null && item.finished_at !== null,
		RECENT_TIMED
	);
	if (timed.length < MIN_SAMPLES) return null;

	const took = (item: QueueItem) => Math.max(1, (item.finished_at ?? 0) - (item.started_at ?? 0));
	const rate = (item: QueueItem) => took(item) / item.downloaded;
	const usual = median(timed.map(rate));
	const kept = timed.filter((item) => rate(item) <= usual * OUTLIER);

	const perTrack = sum(kept.map(took)) / sum(kept.map((item) => item.downloaded));
	const rates = kept.map(rate);
	const uncertainty = Math.min(0.5, spread(rates) / mean(rates) / Math.sqrt(kept.length));

	const weighed = kept.filter((item) => item.bytes > 0);
	const megabytes = weighed.length
		? sum(weighed.map((item) => item.bytes)) / sum(weighed.map((item) => item.downloaded)) / 1e6
		: 0;
	return { perTrack, uncertainty, megabytes, samples: kept.length };
}

interface Sizes {
	/** Tracks an album has, on average, and how much that varies from one album to the next. */
	mean: number;
	spread: number;
	/** The share of those tracks that had to be written (the others were on the disk already). */
	written: number;
	/** The share of albums that wrote at least one file. */
	wrote: number;
	samples: number;
}

function learnSizes(queue: readonly QueueItem[]): Sizes | null {
	const sized = lastAlbums(queue, (item) => item.downloaded + item.skipped > 0, RECENT_SIZED);
	if (sized.length < MIN_SAMPLES) return null;

	const tracks = sized.map((item) => item.downloaded + item.skipped);
	return {
		mean: mean(tracks),
		spread: spread(tracks),
		written: Math.min(1, Math.max(0.05, sum(sized.map((item) => item.downloaded)) / sum(tracks))),
		wrote: sized.filter((item) => item.downloaded > 0).length / sized.length,
		samples: sized.length
	};
}

/** What the queue still has to do, as far as its size can be told. */
interface Work {
	/** Albums waiting, whose size is the usual one. */
	albums: number;
	/** Single tracks waiting. */
	tracks: number;
	/** Tracks the album in progress has left to write, when it is known. */
	remaining: number;
	uncounted: number;
}

function workLeft(queue: readonly QueueItem[], progress: Progress, written: number): Work {
	const work: Work = { albums: 0, tracks: 0, remaining: 0, uncounted: 0 };
	for (const item of queue) {
		if (item.status === 'running') {
			const card = progress.item_id === item.id ? progress.album : null;
			if (item.kind === 'album' && card) {
				const left = Math.max(0, card.tracks_count - item.downloaded - item.skipped);
				work.remaining = left * written;
			} else if (item.kind === 'album') {
				work.albums += 1;
			} else if (item.kind === 'track') {
				work.tracks += 1;
			} else {
				work.uncounted += 1;
			}
		} else if (item.status === 'pending') {
			if (item.kind === 'album') work.albums += 1;
			else if (item.kind === 'track') work.tracks += 1;
			else work.uncounted += 1;
		}
	}
	return work;
}

const average = (bounds: { min_ms: number; max_ms: number }) => (bounds.min_ms + bounds.max_ms) / 2000;

/**
 * Goes through the work as the worker will, album after album: the time of the tracks, the wait
 * between two albums, and a long break whenever one of its conditions is reached.
 */
function simulate(
	work: Work,
	limits: RateLimit,
	pace: { perTrack: number; megabytes: number },
	sizes: { albumTracks: number; written: number; wrote: number }
): { seconds: number; breaks: number } {
	const gap =
		sizes.wrote * average(limits.between_albums) + (1 - sizes.wrote) * (limits.between_albums.min_ms / 1000);
	const rest = average(limits.break_length);

	let seconds = work.remaining * pace.perTrack;
	let breaks = 0;
	let tracks = work.remaining;
	let albums = 0;
	let megabytes = work.remaining * pace.megabytes;

	const reached = () =>
		(limits.break_after_tracks > 0 && tracks >= limits.break_after_tracks) ||
		(limits.break_after_albums > 0 && albums >= limits.break_after_albums) ||
		(limits.break_after_megabytes > 0 && megabytes >= limits.break_after_megabytes);

	const items = work.albums + work.tracks;
	for (let at = 0; at < items; at++) {
		const written = at < work.albums ? sizes.albumTracks * sizes.written : 1;
		seconds += written * pace.perTrack;
		tracks += written;
		albums += 1;
		megabytes += written * pace.megabytes;

		// Nothing comes after the last one: no wait, no break
		if (at === items - 1) break;
		if (reached()) {
			seconds += rest;
			breaks += 1;
			tracks = 0;
			albums = 0;
			megabytes = 0;
		} else {
			seconds += gap;
		}
	}
	return { seconds, breaks };
}

/** How long the queue needs, or `null` while it has nothing to go by or nothing left to do. */
export function estimate({ queue, progress, limits }: Input): Estimate | null {
	const pace = learnPace(queue);
	const sizes = learnSizes(queue);
	if (!pace || !sizes) return null;

	const work = workLeft(queue, progress, sizes.written);
	if (work.albums + work.tracks === 0 && work.remaining === 0) return null;

	const run = (albumTracks: number, perTrack: number) =>
		simulate(work, limits, { perTrack, megabytes: pace.megabytes }, {
			albumTracks,
			written: sizes.written,
			wrote: sizes.wrote
		});

	// The sum of many albums varies less than one album does: the range closes as the queue grows
	const waiting = Math.max(1, work.albums);
	const margin = (Z * sizes.spread) / Math.sqrt(waiting);
	const timing = Z * pace.uncertainty;
	const expected = run(sizes.mean, pace.perTrack);
	const low = run(Math.max(1, sizes.mean - margin), pace.perTrack * Math.max(0.3, 1 - timing));
	const high = run(sizes.mean + margin, pace.perTrack * (1 + timing));

	return {
		seconds: expected.seconds,
		low: Math.min(low.seconds, expected.seconds),
		high: Math.max(high.seconds, expected.seconds),
		uncounted: work.uncounted,
		basis: {
			albums: Math.min(pace.samples, sizes.samples),
			secondsPerTrack: pace.perTrack,
			tracksPerAlbum: sizes.mean,
			breaks: expected.breaks
		}
	};
}
