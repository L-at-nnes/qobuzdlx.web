import type { Progress, QueueItem, RateLimit, Sample, Work } from './types.ts';

/**
 * How long the queue still needs, learned from the albums it has already finished.
 *
 * A queue mixes albums of ten tracks and albums of five hundred, and nothing says which is which
 * before it starts. The albums that wait are therefore drawn, at random, from the albums that were
 * done, and the whole queue is played through as the worker will go through it: the time of the
 * tracks, the wait between two albums, a long break whenever one of its conditions is reached. That
 * is done many times, and the answer is the mean of the runs with the range that holds 80 % of
 * them. Playing the queue through, rather than multiplying averages, is what lets a few huge albums
 * trigger their breaks sooner than an average album would.
 *
 * The wait before each track and the speed of the transfers are not in the settings' arithmetic:
 * they are part of how long a track took, and so are learned. The waits between albums and the long
 * breaks come from the settings, so that changing them changes the figure at once.
 */

/** The width of the range: 80 % of the runs lie within it. */
const LOW = 0.1;
const HIGH = 0.9;
/** Fewer finished albums than this and any figure would be a guess about a guess. */
const MIN_SAMPLES = 3;
/** Fewer than this and the figure is given as rough. */
const SOLID_SAMPLES = 10;
/** The latest albums looked at for the speed: those that the current settings produced. */
const RECENT_TIMED = 40;
/** An album that took this many times the usual time per track was interrupted (a pause, a sign-out). */
const OUTLIER = 4;
/** About how many steps the runs take together: a queue of a hundred thousand is played fewer times. */
const STEPS = 3_000_000;
const MIN_RUNS = 20;
const MAX_RUNS = 400;

export interface Estimate {
	/** Seconds left, as expected, and the two ends of the range. */
	seconds: number;
	low: number;
	high: number;
	/** Items waiting whose size cannot be guessed (a discography, a playlist): they are not counted. */
	uncounted: number;
	/** Few albums to go by: the figure is rough. */
	rough: boolean;
	/** What the figure rests on. */
	basis: {
		albums: number;
		secondsPerTrack: number;
		tracksPerAlbum: number;
		/** The long breaks that the downloads run into, on average. */
		breaks: number;
	};
}

interface Input {
	/** The finished albums, oldest first, as the server gives them. */
	history: readonly Sample[];
	/** What waits, counted by how well its size is known. */
	work: Work;
	/** The item being downloaded, if any. */
	running: QueueItem | null;
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

/** A small generator that always draws the same numbers: a figure must not jitter on its own. */
function generator(seed: number) {
	let state = seed >>> 0;
	return () => {
		state = (state + 0x6d2b79f5) >>> 0;
		let value = state;
		value = Math.imul(value ^ (value >>> 15), value | 1);
		value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
		return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
	};
}

interface Pace {
	/** Seconds to write a track, and how sure that is (relative). */
	perTrack: number;
	uncertainty: number;
	/** Seconds an album takes when it finds everything already in place. */
	overhead: number;
	samples: number;
}

/** How long a written track takes, from the latest albums that wrote some. */
function learnPace(history: readonly Sample[]): Pace | null {
	const timed = history
		.filter((sample) => sample.downloaded > 0 && sample.seconds !== null)
		.slice(-RECENT_TIMED);
	if (timed.length < MIN_SAMPLES) return null;

	const took = (sample: Sample) => Math.max(1, sample.seconds ?? 0);
	const rate = (sample: Sample) => took(sample) / sample.downloaded;
	const usual = median(timed.map(rate));
	const kept = timed.filter((sample) => rate(sample) <= usual * OUTLIER);

	const perTrack = sum(kept.map(took)) / sum(kept.map((sample) => sample.downloaded));
	const rates = kept.map(rate);
	const uncertainty = Math.min(0.5, spread(rates) / mean(rates) / Math.sqrt(kept.length));

	const quick = history
		.filter((sample) => sample.downloaded === 0 && sample.seconds !== null)
		.map(took);
	return { perTrack, uncertainty, overhead: quick.length ? median(quick) : 0, samples: kept.length };
}

/** An album as the runs draw it. */
interface Draw {
	/** Tracks written. */
	written: number;
	megabytes: number;
}

const average = (bounds: { min_ms: number; max_ms: number }) => (bounds.min_ms + bounds.max_ms) / 2000;

/** How many times the queue is played through: fewer for a long queue, whose runs differ little. */
const runsFor = (items: number) =>
	Math.min(MAX_RUNS, Math.max(MIN_RUNS, Math.round(STEPS / Math.max(1, items))));

/** How long the queue needs, or `null` while it has nothing to go by or nothing left to do. */
export function estimate({ history, work, running, progress, limits }: Input): Estimate | null {
	const pace = learnPace(history);
	const pool = history.filter((sample) => sample.downloaded + sample.skipped > 0);
	if (!pace || pool.length < MIN_SAMPLES) return null;

	const tracks = pool.map((sample) => sample.downloaded + sample.skipped);
	const writtenShare = Math.min(
		1,
		Math.max(0.05, sum(pool.map((sample) => sample.downloaded)) / sum(tracks))
	);
	const weighed = pool.filter((sample) => sample.bytes > 0 && sample.downloaded > 0);
	const megabytesPerTrack = weighed.length
		? sum(weighed.map((sample) => sample.bytes)) / sum(weighed.map((sample) => sample.downloaded)) / 1e6
		: 0;
	const draws: Draw[] = pool.map((sample) => ({
		written: sample.downloaded,
		megabytes: sample.bytes > 0 ? sample.bytes / 1e6 : sample.downloaded * megabytesPerTrack
	}));

	// What the item in progress has left, and whether it counts among the albums to draw
	let remaining = 0;
	let albums = work.albums;
	let uncounted = work.uncounted;
	let single = work.tracks;
	if (running) {
		const card = progress.item_id === running.id ? progress.album : null;
		if (running.kind === 'album' && card) {
			remaining = Math.max(0, card.tracks_count - running.downloaded - running.skipped) * writtenShare;
		} else if (running.kind === 'album') {
			albums += 1;
		} else if (running.kind === 'track') {
			single += 1;
		} else {
			uncounted += 1;
		}
	}
	const items = albums + single;
	if (items === 0 && remaining === 0) return null;

	const gap = average(limits.between_albums);
	const shortGap = limits.between_albums.min_ms / 1000;
	const rest = average(limits.break_length);
	const random = generator(pool.length * 7919 + items);
	const normal = () =>
		Math.sqrt(-2 * Math.log(1 - random())) * Math.cos(2 * Math.PI * random());

	const totals: number[] = [];
	let breaksSeen = 0;
	const runs = runsFor(items);
	for (let run = 0; run < runs; run++) {
		// How fast the tracks go is one thing for the whole run: a wrong guess is wrong for all of them
		const perTrack = pace.perTrack * Math.max(0.3, 1 + normal() * pace.uncertainty);
		let seconds = remaining * perTrack;
		let tracksSince = remaining;
		let albumsSince = 0;
		let megabytesSince = remaining * megabytesPerTrack;

		for (let at = 0; at < items; at++) {
			const draw =
				at < albums
					? draws[Math.floor(random() * draws.length)]
					: { written: 1, megabytes: megabytesPerTrack };
			const wrote = draw.written > 0;
			seconds += wrote ? draw.written * perTrack : pace.overhead;
			tracksSince += draw.written;
			albumsSince += wrote ? 1 : 0;
			megabytesSince += draw.megabytes;

			// Nothing comes after the last one: no wait, no break
			if (at === items - 1) break;
			const reached =
				(limits.break_after_tracks > 0 && tracksSince >= limits.break_after_tracks) ||
				(limits.break_after_albums > 0 && albumsSince >= limits.break_after_albums) ||
				(limits.break_after_megabytes > 0 && megabytesSince >= limits.break_after_megabytes);
			if (reached) {
				seconds += rest;
				breaksSeen += 1;
				tracksSince = 0;
				albumsSince = 0;
				megabytesSince = 0;
			} else {
				seconds += wrote ? gap : shortGap;
			}
		}
		totals.push(seconds);
	}

	totals.sort((a, b) => a - b);
	const at = (share: number) => totals[Math.min(totals.length - 1, Math.floor(share * totals.length))];
	const expected = mean(totals);
	return {
		seconds: expected,
		low: Math.min(at(LOW), expected),
		high: Math.max(at(HIGH), expected),
		uncounted,
		rough: Math.min(pace.samples, pool.length) < SOLID_SAMPLES,
		basis: {
			albums: Math.min(pace.samples, pool.length),
			secondsPerTrack: pace.perTrack,
			tracksPerAlbum: mean(tracks),
			breaks: breaksSeen / runs
		}
	};
}
