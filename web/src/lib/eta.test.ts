import { describe, expect, it } from 'vitest';
import { estimate } from './eta';
import type { Progress, QueueItem, RateLimit, Sample, Work } from './types';

const NO_WAITS: RateLimit = {
	between_tracks: { min_ms: 0, max_ms: 0 },
	between_albums: { min_ms: 5000, max_ms: 5000 },
	break_after_tracks: 0,
	break_after_albums: 0,
	break_after_megabytes: 0,
	break_length: { min_ms: 180_000, max_ms: 180_000 },
	cooldown_ms: 60_000
};

const IDLE: Progress = {
	running: false,
	item_id: null,
	percent: 0,
	album: null,
	tracks: [],
	halt: null,
	wait: null
};

/** An album that was written: `tracks` files, `seconds` from start to end, 30 MB each. */
const done = (tracks: number, seconds: number, extra: Partial<Sample> = {}): Sample => ({
	downloaded: tracks,
	skipped: 0,
	seconds,
	bytes: tracks * 30_000_000,
	...extra
});

/** Ten albums of ten tracks, two seconds a track. */
const history = (): Sample[] => Array.from({ length: 10 }, () => done(10, 20));

const waiting = (albums: number, extra: Partial<Work> = {}): Work => ({
	albums,
	tracks: 0,
	uncounted: 0,
	...extra
});

const run = (over: {
	history?: Sample[];
	work?: Work;
	running?: QueueItem | null;
	progress?: Progress;
	limits?: Partial<RateLimit>;
}) =>
	estimate({
		history: over.history ?? history(),
		work: over.work ?? waiting(100),
		running: over.running ?? null,
		progress: over.progress ?? IDLE,
		limits: { ...NO_WAITS, ...over.limits }
	});

describe('the time left', () => {
	it('is not guessed from fewer than three finished albums', () => {
		expect(run({ history: [done(10, 20), done(10, 20)] })).toBeNull();
	});

	it('is nothing when nothing is left', () => {
		expect(run({ work: waiting(0) })).toBeNull();
	});

	it('is the time of the tracks and the wait between albums', () => {
		const result = run({})!;

		// 100 albums of 10 tracks at 2 s, and 99 waits of 5 s between them
		expect(result.seconds).toBeCloseTo(100 * 20 + 99 * 5, 0);
		expect(result.basis.secondsPerTrack).toBeCloseTo(2);
		expect(result.basis.tracksPerAlbum).toBeCloseTo(10);
		expect(result.uncounted).toBe(0);
	});

	it('takes a long break in place of the wait whenever a condition is reached', () => {
		const result = run({ limits: { break_after_albums: 10 } })!;

		// Nine breaks of three minutes, replacing nine of the 99 waits
		expect(result.basis.breaks).toBeCloseTo(9);
		expect(result.seconds).toBeCloseTo(100 * 20 + 90 * 5 + 9 * 180, 0);
	});

	it('breaks as well when the tracks or the megabytes reach their limit first', () => {
		const few = waiting(40);

		const byTracks = run({ work: few, limits: { break_after_tracks: 100 } })!;
		// 10 tracks of 30 MB each: 1000 MB is 33 tracks, so a break every four albums
		const byMegabytes = run({ work: few, limits: { break_after_megabytes: 1000 } })!;

		expect(byTracks.basis.breaks).toBeCloseTo(3);
		expect(byMegabytes.basis.breaks).toBeCloseTo(9);
	});

	it('counts only what is left of the album in progress', () => {
		const running = { id: 'r', kind: 'album', downloaded: 30, skipped: 0 } as QueueItem;
		const progress: Progress = {
			...IDLE,
			running: true,
			item_id: 'r',
			album: {
				title: 'A long one',
				artist: 'Someone',
				release_date: null,
				tracks_count: 40,
				genre: '',
				label: '',
				quality: '',
				cover: ''
			}
		};

		const result = run({ work: waiting(0), running, progress })!;

		// Ten tracks of the forty are left
		expect(result.seconds).toBeCloseTo(10 * 2, 0);
	});

	it('leaves out what cannot be sized and says how many there are', () => {
		const result = run({ work: waiting(10, { uncounted: 2 }) })!;

		expect(result.uncounted).toBe(2);
		expect(result.seconds).toBeCloseTo(10 * 20 + 9 * 5, 0);
	});

	it('counts the single tracks that wait as one file each', () => {
		const result = run({ work: { albums: 0, tracks: 5, uncounted: 0 } })!;

		expect(result.seconds).toBeCloseTo(5 * 2 + 4 * 5, 0);
	});

	it('counts the albums that were already on the disk as quick ones', () => {
		const onDisk = Array.from({ length: 10 }, () => done(0, 2, { skipped: 10, bytes: 0 }));

		const mixed = run({ history: [...history(), ...onDisk], work: waiting(20) })!;
		const all = run({ work: waiting(20) })!;

		// Half of the albums are expected to be there already, and to cost two seconds
		expect(mixed.seconds).toBeLessThan(all.seconds * 0.7);
	});

	it('is not thrown off by an album that was interrupted', () => {
		const normal = run({ work: waiting(50) })!;
		// One album that was paused for an hour in the middle
		const result = run({ history: [done(10, 3600), ...history()], work: waiting(50) })!;

		expect(result.basis.secondsPerTrack).toBeCloseTo(normal.basis.secondsPerTrack, 1);
	});

	it('says it is rough while there are few albums to go by', () => {
		expect(run({ history: history().slice(0, 4) })!.rough).toBe(true);
		expect(run({})!.rough).toBe(false);
	});

	it('follows the settings at once, the waits being theirs', () => {
		const calm = run({})!;
		const patient = run({
			limits: { between_albums: { min_ms: 60_000, max_ms: 60_000 } }
		})!;

		expect(patient.seconds - calm.seconds).toBeCloseTo(99 * 55, 0);
	});

	it('is the same every time it is asked', () => {
		const uneven = [...history(), done(500, 1000), done(300, 600)];

		expect(run({ history: uneven })!.seconds).toBe(run({ history: uneven })!.seconds);
	});
});

describe('the range of the time left', () => {
	/** Albums of ten tracks, and one in ten of five hundred. */
	const uneven = () =>
		Array.from({ length: 50 }, (_, at) => (at % 10 === 0 ? done(500, 1000) : done(10, 20)));

	it('surrounds the expected time', () => {
		const result = run({ history: uneven(), work: waiting(30) })!;

		expect(result.low).toBeLessThan(result.seconds);
		expect(result.high).toBeGreaterThan(result.seconds);
	});

	it('is wider, in proportion, for a few albums than for thousands', () => {
		const few = run({ history: uneven(), work: waiting(5) })!;
		const many = run({ history: uneven(), work: waiting(2000) })!;

		const width = (result: typeof few) => (result.high - result.low) / result.seconds;
		expect(width(few)).toBeGreaterThan(width(many) * 3);
	});

	it('is nothing but the expected time when every album is alike', () => {
		const result = run({ work: waiting(10) })!;

		expect(result.high - result.low).toBeLessThan(1);
	});

	it('follows the average size of what was done: five hundred tracks now and then', () => {
		const result = run({ history: uneven(), work: waiting(1000) })!;

		// 59 tracks an album on average, at two seconds each
		expect(result.basis.tracksPerAlbum).toBeCloseTo(59, 0);
		expect(result.seconds).toBeGreaterThan(1000 * 59 * 2 * 0.95);
		expect(result.seconds).toBeLessThan(1000 * 59 * 2 * 1.1);
	});

	it('knows that a few huge albums bring the breaks sooner than the average one', () => {
		// 1000 MB: an album of ten tracks weighs 300 MB, one of five hundred weighs 15 000 MB
		const limits = { break_after_megabytes: 1000 };

		const result = run({ history: uneven(), work: waiting(200), limits })!;

		// Each huge album (one in ten) is a break of its own, and the small ones add one every four
		expect(result.basis.breaks).toBeGreaterThan(200 * 0.1);
	});
});

describe('a queue of a hundred thousand albums', () => {
	it('is estimated at once, and closely', () => {
		const started = performance.now();

		const result = run({
			history: Array.from({ length: 200 }, (_, at) => (at % 10 === 0 ? done(500, 1000) : done(10, 20))),
			work: waiting(100_000),
			limits: { break_after_albums: 10 }
		})!;

		expect(performance.now() - started).toBeLessThan(1500);
		// 59 tracks an album at two seconds, a wait or a break after each, on average
		expect(result.seconds).toBeGreaterThan(100_000 * 59 * 2);
		expect(result.high - result.low).toBeLessThan(result.seconds * 0.02);
	});
});
