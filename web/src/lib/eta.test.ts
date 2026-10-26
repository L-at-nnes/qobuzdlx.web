import { describe, expect, it } from 'vitest';
import { estimate } from './eta';
import type { ItemStatus, Progress, QueueItem, RateLimit } from './types';

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

let counter = 0;

function item(kind: string, status: ItemStatus, extra: Partial<QueueItem> = {}): QueueItem {
	counter += 1;
	return {
		id: `i${counter}`,
		url: `https://play.qobuz.com/album/${counter}`,
		kind,
		ref_id: String(counter),
		label: `Album ${counter}`,
		sub_label: null,
		thumbnail: null,
		status,
		error: null,
		enqueued_at: 0,
		started_at: null,
		finished_at: null,
		downloaded: 0,
		skipped: 0,
		bytes: 0,
		failures: [],
		...extra
	};
}

/** An album that was written: `tracks` files, `seconds` from start to end. */
function done(tracks: number, seconds: number, extra: Partial<QueueItem> = {}): QueueItem {
	return item('album', 'completed', {
		downloaded: tracks,
		started_at: 1000,
		finished_at: 1000 + seconds,
		bytes: tracks * 30_000_000,
		...extra
	});
}

const waiting = (count: number) => Array.from({ length: count }, () => item('album', 'pending'));

/** Ten albums of ten tracks, two seconds a track. */
const history = () => Array.from({ length: 10 }, () => done(10, 20));

describe('the time left', () => {
	it('is not guessed from fewer than three finished albums', () => {
		const queue = [done(10, 20), done(10, 20), ...waiting(5)];

		expect(estimate({ queue, progress: IDLE, limits: NO_WAITS })).toBeNull();
	});

	it('is nothing when nothing is left', () => {
		expect(estimate({ queue: history(), progress: IDLE, limits: NO_WAITS })).toBeNull();
	});

	it('is the time of the tracks and the wait between albums', () => {
		const queue = [...history(), ...waiting(100)];

		const result = estimate({ queue, progress: IDLE, limits: NO_WAITS })!;

		// 100 albums of 10 tracks at 2 s, and 99 waits of 5 s between them
		expect(result.seconds).toBeCloseTo(100 * 20 + 99 * 5, 0);
		expect(result.basis.secondsPerTrack).toBeCloseTo(2);
		expect(result.basis.tracksPerAlbum).toBeCloseTo(10);
		expect(result.uncounted).toBe(0);
	});

	it('takes a long break in place of the wait whenever a condition is reached', () => {
		const queue = [...history(), ...waiting(100)];
		const limits = { ...NO_WAITS, break_after_albums: 10 };

		const result = estimate({ queue, progress: IDLE, limits })!;

		// Nine breaks of three minutes, replacing nine of the 99 waits
		expect(result.basis.breaks).toBe(9);
		expect(result.seconds).toBeCloseTo(100 * 20 + 90 * 5 + 9 * 180, 0);
	});

	it('breaks as well when the tracks or the megabytes reach their limit first', () => {
		const queue = [...history(), ...waiting(40)];

		const byTracks = estimate({ queue, progress: IDLE, limits: { ...NO_WAITS, break_after_tracks: 100 } })!;
		// 10 tracks of 30 MB each: 1000 MB is 33 tracks, so a break every four albums
		const byMegabytes = estimate({
			queue,
			progress: IDLE,
			limits: { ...NO_WAITS, break_after_megabytes: 1000 }
		})!;

		expect(byTracks.basis.breaks).toBe(3);
		expect(byMegabytes.basis.breaks).toBe(9);
	});

	it('counts only what is left of the album in progress', () => {
		const running = item('album', 'running', { downloaded: 30, skipped: 0 });
		const progress: Progress = {
			...IDLE,
			running: true,
			item_id: running.id,
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

		const result = estimate({ queue: [...history(), running], progress, limits: NO_WAITS })!;

		// Ten tracks of the forty are left
		expect(result.seconds).toBeCloseTo(10 * 2, 0);
	});

	it('leaves out what cannot be sized and says how many there are', () => {
		const queue = [...history(), ...waiting(10), item('artist', 'pending'), item('playlist', 'pending')];

		const result = estimate({ queue, progress: IDLE, limits: NO_WAITS })!;

		expect(result.uncounted).toBe(2);
		expect(result.seconds).toBeCloseTo(10 * 20 + 9 * 5, 0);
	});

	it('counts the albums that were already on the disk as quick ones', () => {
		const onDisk = Array.from({ length: 10 }, () =>
			item('album', 'completed', { downloaded: 0, skipped: 10, started_at: 1000, finished_at: 1002 })
		);
		const queue = [...history(), ...onDisk, ...waiting(20)];

		const mixed = estimate({ queue, progress: IDLE, limits: NO_WAITS })!;
		const all = estimate({ queue: [...history(), ...waiting(20)], progress: IDLE, limits: NO_WAITS })!;

		// Half of the tracks are expected to be there already
		expect(mixed.seconds).toBeLessThan(all.seconds);
	});

	it('is not thrown off by an album that was interrupted', () => {
		const normal = estimate({ queue: [...history(), ...waiting(50)], progress: IDLE, limits: NO_WAITS })!;
		// One album that was paused for an hour in the middle
		const queue = [done(10, 3600), ...history(), ...waiting(50)];

		const result = estimate({ queue, progress: IDLE, limits: NO_WAITS })!;

		expect(result.basis.secondsPerTrack).toBeCloseTo(normal.basis.secondsPerTrack, 1);
	});
});

describe('the range of the time left', () => {
	/** Albums of ten tracks, and one in ten of five hundred. */
	const uneven = () =>
		Array.from({ length: 50 }, (_, at) => (at % 10 === 0 ? done(500, 1000) : done(10, 20)));

	it('surrounds the expected time', () => {
		const result = estimate({ queue: [...uneven(), ...waiting(30)], progress: IDLE, limits: NO_WAITS })!;

		expect(result.low).toBeLessThan(result.seconds);
		expect(result.high).toBeGreaterThan(result.seconds);
	});

	it('is wider, in proportion, for a few albums than for thousands', () => {
		const few = estimate({ queue: [...uneven(), ...waiting(5)], progress: IDLE, limits: NO_WAITS })!;
		const many = estimate({ queue: [...uneven(), ...waiting(2000)], progress: IDLE, limits: NO_WAITS })!;

		const width = (result: typeof few) => (result.high - result.low) / result.seconds;
		expect(width(few)).toBeGreaterThan(width(many) * 3);
	});

	it('is nothing but the expected time when every album is alike', () => {
		const result = estimate({ queue: [...history(), ...waiting(10)], progress: IDLE, limits: NO_WAITS })!;

		expect(result.high - result.low).toBeLessThan(1);
	});

	it('follows the average size of what was done: five hundred tracks now and then', () => {
		const result = estimate({ queue: [...uneven(), ...waiting(100)], progress: IDLE, limits: NO_WAITS })!;

		// 59 tracks an album on average, at two seconds each
		expect(result.basis.tracksPerAlbum).toBeCloseTo(59, 0);
		expect(result.seconds).toBeGreaterThan(100 * 59 * 2 * 0.95);
		expect(result.seconds).toBeLessThan(100 * 59 * 2 * 1.1);
	});
});
