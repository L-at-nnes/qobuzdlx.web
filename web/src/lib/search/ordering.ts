import type { AlbumItem, ArtistItem, PlaylistItem, TrackItem } from '../types';

/**
 * How a list of results is ordered. Qobuz has no sort parameter, so the results already fetched are
 * ordered here. `relevance` is the order the server gave, and is always recoverable: nothing below
 * changes its input.
 */
export type Sort =
	| 'relevance'
	| 'titleAsc'
	| 'titleDesc'
	| 'artistAsc'
	| 'artistDesc'
	| 'albumAsc'
	| 'nameAsc'
	| 'nameDesc'
	| 'dateDesc'
	| 'dateAsc'
	| 'qualityDesc'
	| 'tracksDesc'
	| 'tracksAsc'
	| 'albumsDesc'
	| 'albumsAsc'
	| 'labelAsc'
	| 'durationAsc'
	| 'durationDesc';

/** The orderings that make sense for each list. */
export const SORTS = {
	albums: [
		'relevance',
		'titleAsc',
		'titleDesc',
		'artistAsc',
		'artistDesc',
		'dateDesc',
		'dateAsc',
		'qualityDesc',
		'tracksDesc',
		'tracksAsc',
		'labelAsc'
	],
	tracks: [
		'relevance',
		'titleAsc',
		'titleDesc',
		'artistAsc',
		'albumAsc',
		'durationAsc',
		'durationDesc',
		'qualityDesc',
		'dateDesc'
	],
	artists: ['relevance', 'nameAsc', 'nameDesc', 'albumsDesc', 'albumsAsc'],
	playlists: ['relevance']
} as const satisfies Record<string, readonly Sort[]>;

/** Compares text the way a person sorts a shelf: accents and case ignored, numbers by value. */
function collator(language: string): Intl.Collator {
	return new Intl.Collator(language, { sensitivity: 'base', numeric: true });
}

/** Dates are ISO strings and sometimes missing. Missing ones go last, whichever way it is sorted. */
function byDate<T>(items: T[], date: (item: T) => string | null, direction: 1 | -1): T[] {
	const time = (item: T) => {
		const value = date(item);
		const parsed = value ? Date.parse(value) : Number.NaN;
		return Number.isNaN(parsed) ? null : parsed;
	};
	return [...items].sort((a, b) => {
		const left = time(a);
		const right = time(b);
		if (left === null && right === null) return 0;
		if (left === null) return 1;
		if (right === null) return -1;
		return (left - right) * direction;
	});
}

function byQuality<T>(items: T[], depth: (item: T) => number, rate: (item: T) => number): T[] {
	return [...items].sort((a, b) => depth(b) - depth(a) || rate(b) - rate(a));
}

export function sortAlbums(items: AlbumItem[], sort: Sort, language = 'en'): AlbumItem[] {
	const text = collator(language);
	const by = (pick: (item: AlbumItem) => string, direction: 1 | -1 = 1) =>
		[...items].sort((a, b) => text.compare(pick(a), pick(b)) * direction);

	switch (sort) {
		case 'titleAsc':
			return by((album) => album.title);
		case 'titleDesc':
			return by((album) => album.title, -1);
		case 'artistAsc':
			return by((album) => album.artist);
		case 'artistDesc':
			return by((album) => album.artist, -1);
		case 'labelAsc':
			return by((album) => album.label);
		case 'dateDesc':
			return byDate(items, (album) => album.release_date, -1);
		case 'dateAsc':
			return byDate(items, (album) => album.release_date, 1);
		case 'qualityDesc':
			return byQuality(
				items,
				(album) => album.bit_depth,
				(album) => album.sampling_rate
			);
		case 'tracksDesc':
			return [...items].sort((a, b) => b.tracks_count - a.tracks_count);
		case 'tracksAsc':
			return [...items].sort((a, b) => a.tracks_count - b.tracks_count);
		default:
			return items;
	}
}

export function sortTracks(items: TrackItem[], sort: Sort, language = 'en'): TrackItem[] {
	const text = collator(language);
	const by = (pick: (item: TrackItem) => string, direction: 1 | -1 = 1) =>
		[...items].sort((a, b) => text.compare(pick(a), pick(b)) * direction);

	switch (sort) {
		case 'titleAsc':
			return by((track) => track.title);
		case 'titleDesc':
			return by((track) => track.title, -1);
		case 'artistAsc':
			return by((track) => track.artist);
		case 'albumAsc':
			return by((track) => track.album?.title ?? '');
		case 'dateDesc':
			return byDate(items, (track) => track.album?.release_date ?? null, -1);
		case 'qualityDesc':
			return byQuality(
				items,
				(track) => track.bit_depth,
				(track) => track.sampling_rate
			);
		case 'durationAsc':
			return [...items].sort((a, b) => a.duration - b.duration);
		case 'durationDesc':
			return [...items].sort((a, b) => b.duration - a.duration);
		default:
			return items;
	}
}

export function sortArtists(items: ArtistItem[], sort: Sort, language = 'en'): ArtistItem[] {
	const text = collator(language);
	switch (sort) {
		case 'nameAsc':
			return [...items].sort((a, b) => text.compare(a.name, b.name));
		case 'nameDesc':
			return [...items].sort((a, b) => text.compare(b.name, a.name));
		case 'albumsDesc':
			return [...items].sort((a, b) => b.albums_count - a.albums_count);
		case 'albumsAsc':
			return [...items].sort((a, b) => a.albums_count - b.albums_count);
		default:
			return items;
	}
}

export function sortPlaylists(items: PlaylistItem[]): PlaylistItem[] {
	return items;
}

// ─── Quality ───

/**
 * Hi-Res in the usual sense of Qobuz: more than a CD, in depth or in rate. A value that is missing
 * (0) does not make a release Hi-Res.
 */
export function isHiRes(bitDepth: number, samplingRate: number): boolean {
	return bitDepth > 16 || samplingRate > 44.1;
}

/** `24-bit / 96 kHz`, or nothing when Qobuz says nothing. */
export function formatQuality(bitDepth: number, samplingRate: number): string {
	if (!bitDepth || !samplingRate) return '';
	return `${bitDepth}-bit / ${Number(samplingRate)} kHz`;
}

/** `24/96`: short enough for a chip. */
export function shortQuality(bitDepth: number, samplingRate: number): string {
	if (!bitDepth || !samplingRate) return '';
	return `${bitDepth}/${Number(samplingRate)}`;
}

// ─── Text ───

/** `4:09`, or `1:02:03` past an hour. */
export function formatDuration(seconds: number): string {
	const total = Math.max(0, Math.round(seconds));
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	const rest = String(total % 60).padStart(2, '0');
	return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
}

/** The year of an ISO date, or nothing. */
export function releaseYear(date: string | null | undefined): string {
	const match = date?.match(/^(\d{4})/);
	return match ? match[1] : '';
}

/** `Title (Version)`, with a version already in the title not said twice. */
export function withVersion(title: string, version: string | null | undefined): string {
	if (!version) return title;
	return title.toLowerCase().includes(`(${version.toLowerCase()})`) ? title : `${title} (${version})`;
}

/** A track number the way a record sleeve shows it: `01`, or `1` on a short album. */
export function trackNumber(number: number): string {
	return String(number).padStart(2, '0');
}
