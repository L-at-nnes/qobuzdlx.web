import { describe, expect, it } from 'vitest';
import type { AlbumItem, ArtistItem, TrackItem } from '../types';
import {
	formatDuration,
	formatQuality,
	isHiRes,
	releaseYear,
	shortQuality,
	sortAlbums,
	sortArtists,
	sortTracks,
	trackNumber,
	withVersion
} from './ordering';

function album(over: Partial<AlbumItem>): AlbumItem {
	return {
		id: 'a',
		url: '',
		title: 'Title',
		version: null,
		artist: 'Artist',
		release_date: null,
		tracks_count: 10,
		bit_depth: 16,
		sampling_rate: 44.1,
		label: 'Label',
		thumbnail: '',
		parental_warning: false,
		...over
	};
}

function track(over: Partial<TrackItem>): TrackItem {
	return {
		id: 1,
		url: '',
		title: 'Title',
		version: null,
		number: 1,
		disc: 1,
		artist: 'Artist',
		album: null,
		duration: 100,
		bit_depth: 16,
		sampling_rate: 44.1,
		parental_warning: false,
		...over
	};
}

const titles = (items: { title: string }[]) => items.map((item) => item.title);

describe('sorting albums', () => {
	const albums = [
		album({ title: 'Symphony No. 10', release_date: '1999-01-01', tracks_count: 4 }),
		album({ title: 'symphony no. 2', release_date: null, tracks_count: 9 }),
		album({ title: 'Étude', release_date: '2010-05-05', tracks_count: 1 })
	];

	it('keeps the order of the server for relevance', () => {
		expect(sortAlbums(albums, 'relevance')).toBe(albums);
	});

	it('orders titles like a shelf: numbers by value, case and accents ignored', () => {
		expect(titles(sortAlbums(albums, 'titleAsc'))).toEqual([
			'Étude',
			'symphony no. 2',
			'Symphony No. 10'
		]);
		expect(titles(sortAlbums(albums, 'titleDesc')).at(0)).toBe('Symphony No. 10');
	});

	it('puts undated releases last whichever way dates are ordered', () => {
		expect(titles(sortAlbums(albums, 'dateDesc'))).toEqual([
			'Étude',
			'Symphony No. 10',
			'symphony no. 2'
		]);
		expect(titles(sortAlbums(albums, 'dateAsc'))).toEqual([
			'Symphony No. 10',
			'Étude',
			'symphony no. 2'
		]);
	});

	it('orders by track count both ways', () => {
		expect(sortAlbums(albums, 'tracksDesc').map((a) => a.tracks_count)).toEqual([9, 4, 1]);
		expect(sortAlbums(albums, 'tracksAsc').map((a) => a.tracks_count)).toEqual([1, 4, 9]);
	});

	it('orders by depth, then by rate', () => {
		const mixed = [
			album({ title: 'cd', bit_depth: 16, sampling_rate: 44.1 }),
			album({ title: 'hi 96', bit_depth: 24, sampling_rate: 96 }),
			album({ title: 'hi 192', bit_depth: 24, sampling_rate: 192 })
		];

		expect(titles(sortAlbums(mixed, 'qualityDesc'))).toEqual(['hi 192', 'hi 96', 'cd']);
	});

	it('never changes what it was given', () => {
		const before = [...albums];

		sortAlbums(albums, 'titleAsc');
		sortAlbums(albums, 'dateDesc');

		expect(albums).toEqual(before);
	});

	it('is stable: equal items keep their relevance order', () => {
		const same = [
			album({ id: '1', artist: 'Bach' }),
			album({ id: '2', artist: 'Bach' }),
			album({ id: '3', artist: 'Bach' })
		];

		expect(sortAlbums(same, 'artistAsc').map((a) => a.id)).toEqual(['1', '2', '3']);
	});
});

describe('sorting tracks and artists', () => {
	it('orders tracks by duration', () => {
		const tracks = [track({ title: 'b', duration: 200 }), track({ title: 'a', duration: 50 })];

		expect(titles(sortTracks(tracks, 'durationAsc'))).toEqual(['a', 'b']);
		expect(titles(sortTracks(tracks, 'durationDesc'))).toEqual(['b', 'a']);
	});

	it('orders tracks by the album they come from, tracks without one first', () => {
		const tracks = [
			track({ title: 'x', album: { id: '1', title: 'Zeta', thumbnail: '', release_date: null } }),
			track({ title: 'y', album: null })
		];

		expect(titles(sortTracks(tracks, 'albumAsc'))).toEqual(['y', 'x']);
	});

	it('orders artists by name and by number of albums', () => {
		const artists: ArtistItem[] = [
			{ id: 1, url: '', name: 'Zelenka', picture: null, albums_count: 3 },
			{ id: 2, url: '', name: 'Albinoni', picture: null, albums_count: 30 }
		];

		expect(sortArtists(artists, 'nameAsc').map((a) => a.name)).toEqual(['Albinoni', 'Zelenka']);
		expect(sortArtists(artists, 'albumsAsc').map((a) => a.name)).toEqual(['Zelenka', 'Albinoni']);
	});
});

describe('quality', () => {
	it('calls Hi-Res what is more than a CD in depth or in rate', () => {
		expect(isHiRes(24, 44.1)).toBe(true);
		expect(isHiRes(16, 96)).toBe(true);
		expect(isHiRes(16, 44.1)).toBe(false);
	});

	it('does not call Hi-Res what Qobuz says nothing about', () => {
		expect(isHiRes(0, 0)).toBe(false);
	});

	it('writes the quality in full and in short', () => {
		expect(formatQuality(24, 96)).toBe('24-bit / 96 kHz');
		expect(formatQuality(16, 44.1)).toBe('16-bit / 44.1 kHz');
		expect(shortQuality(24, 192)).toBe('24/192');
		expect(formatQuality(0, 0)).toBe('');
		expect(shortQuality(24, 0)).toBe('');
	});
});

describe('text', () => {
	it('writes durations as a person reads them', () => {
		expect(formatDuration(0)).toBe('0:00');
		expect(formatDuration(249)).toBe('4:09');
		expect(formatDuration(3723)).toBe('1:02:03');
		expect(formatDuration(-5)).toBe('0:00');
	});

	it('takes the year of a date', () => {
		expect(releaseYear('2013-05-17')).toBe('2013');
		expect(releaseYear(null)).toBe('');
		expect(releaseYear('unknown')).toBe('');
	});

	it('adds a version to a title once', () => {
		expect(withVersion('Symphony', 'Live')).toBe('Symphony (Live)');
		expect(withVersion('Symphony (Live)', 'live')).toBe('Symphony (Live)');
		expect(withVersion('Symphony', null)).toBe('Symphony');
	});

	it('pads track numbers to two digits', () => {
		expect(trackNumber(3)).toBe('03');
		expect(trackNumber(12)).toBe('12');
	});
});
