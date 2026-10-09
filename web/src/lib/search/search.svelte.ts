import { ApiError, api } from '#lib/api.ts';
import { t } from '#lib/i18n/index.svelte.ts';
import { toast } from '#lib/state/toast.svelte.ts';
import type {
	AlbumItem,
	ArtistItem,
	PlaylistItem,
	SearchKind,
	TrackItem
} from '#lib/types.ts';
import type { Wanted } from '#lib/state/enqueue.ts';
import type { Sort } from './ordering.ts';
import { Selection } from './selection.svelte.ts';

/** How many results a page holds. */
const PAGE_SIZE = 50;
const STORAGE_KEY = 'qbdlx.search';

interface Found<T> {
	/** What was asked, so that a page of more results is asked the same way. */
	query: string;
	items: T[];
	total: number;
}

type Child =
	| { status: 'loading' }
	| { status: 'failed' }
	| { status: 'ready'; tracks: TrackItem[] }
	| { status: 'ready-albums'; albums: AlbumItem[] };

type Results = {
	albums: Found<AlbumItem> | null;
	tracks: Found<TrackItem> | null;
	artists: Found<ArtistItem> | null;
	playlists: Found<PlaylistItem> | null;
};

const empty = (): Results => ({ albums: null, tracks: null, artists: null, playlists: null });

/** The key an album, a playlist or an artist is opened under. */
export const albumKey = (id: string) => `album:${id}`;
export const playlistKey = (id: number) => `playlist:${id}`;
export const artistKey = (id: number) => `artist:${id}`;

/**
 * The state of the search page. It lives apart from the page so that leaving for the downloads and
 * coming back finds the results where they were, and it is kept for the tab so that a reload does too.
 */
class SearchState {
	query = $state('');
	kind = $state<SearchKind>('albums');
	hiRes = $state(false);
	sorts = $state<Record<SearchKind, Sort>>({
		albums: 'relevance',
		tracks: 'relevance',
		artists: 'relevance',
		playlists: 'relevance'
	});
	/** The sort of the albums of an opened artist. */
	artistAlbumSort = $state<Sort>('dateDesc');

	results = $state<Results>(empty());
	searching = $state(false);
	loadingMore = $state(false);
	failure = $state<string | null>(null);

	/** What is opened, and what was loaded for it. */
	open = $state<Record<string, boolean>>({});
	children = $state<Record<string, Child>>({});

	selectedAlbums = new Selection<string>();
	selectedTracks = new Selection<number>();
	selectionCount = $derived(this.selectedAlbums.size + this.selectedTracks.size);

	/** Whether a search was ever made: the page shows a welcome until one is. */
	asked = $derived(Object.values(this.results).some((found) => found !== null));

	constructor() {
		this.restore();
	}

	current = $derived(this.results[this.kind]);

	async search(kind: SearchKind = this.kind) {
		const query = this.query.trim();
		if (!query) return;
		this.kind = kind;
		this.searching = true;
		this.failure = null;
		try {
			const page = await api.search(kind, query, 0, PAGE_SIZE);
			this.results = {
				...this.results,
				[kind]: { query, items: page.items, total: page.total }
			} as Results;
			this.open = {};
			this.children = {};
			this.persist();
		} catch (error) {
			this.failure = this.explain(error);
		} finally {
			this.searching = false;
		}
	}

	async loadMore() {
		const found = this.results[this.kind];
		if (!found || this.loadingMore || found.items.length >= found.total) return;
		this.loadingMore = true;
		try {
			const page = await api.search(this.kind, found.query, found.items.length, PAGE_SIZE);
			this.results = {
				...this.results,
				[this.kind]: {
					query: found.query,
					items: [...found.items, ...page.items],
					total: page.total
				}
			} as Results;
			this.persist();
		} catch (error) {
			toast.error(this.explain(error));
		} finally {
			this.loadingMore = false;
		}
	}

	private explain(error: unknown): string {
		return error instanceof ApiError && error.status !== 0
			? `${t('search.failed')} ${error.message}`
			: t('search.failed');
	}

	// ─── Opening a row ───

	/** Opens or closes a row, loading what is inside it the first time. */
	async toggle(key: string, load: () => Promise<Child>) {
		const opening = !this.open[key];
		this.open = { ...this.open, [key]: opening };
		if (!opening || this.children[key]?.status === 'ready' || this.children[key]?.status === 'ready-albums') {
			return;
		}
		this.children = { ...this.children, [key]: { status: 'loading' } };
		try {
			this.children = { ...this.children, [key]: await load() };
		} catch {
			this.children = { ...this.children, [key]: { status: 'failed' } };
		}
	}

	async toggleAlbum(id: string) {
		await this.toggle(albumKey(id), async () => ({
			status: 'ready',
			tracks: await api.albumTracks(id)
		}));
	}

	async togglePlaylist(id: number) {
		await this.toggle(playlistKey(id), async () => ({
			status: 'ready',
			tracks: await api.playlistTracks(id)
		}));
	}

	async toggleArtist(id: number) {
		await this.toggle(artistKey(id), async () => ({
			status: 'ready-albums',
			albums: await api.artistAlbums(id)
		}));
	}

	// ─── Selection ───

	clearSelection() {
		this.selectedAlbums.clear();
		this.selectedTracks.clear();
	}

	/** The selection as things to put in the queue. */
	selectedWanted(): Wanted[] {
		return [...this.selectedAlbums.wanted(), ...this.selectedTracks.wanted()];
	}

	// ─── Keeping it for the tab ───

	private persist() {
		try {
			sessionStorage.setItem(
				STORAGE_KEY,
				JSON.stringify({
					query: this.query,
					kind: this.kind,
					hiRes: this.hiRes,
					sorts: this.sorts,
					results: this.results
				})
			);
		} catch {
			// A full or unavailable storage only means the results are not kept across a reload
		}
	}

	/** Remembers what the sort and the filter are, as soon as they change. */
	save() {
		this.persist();
	}

	private restore() {
		try {
			const saved = JSON.parse(sessionStorage.getItem(STORAGE_KEY) ?? 'null');
			if (!saved) return;
			this.query = saved.query ?? '';
			this.kind = saved.kind ?? 'albums';
			this.hiRes = Boolean(saved.hiRes);
			this.sorts = { ...this.sorts, ...saved.sorts };
			this.results = { ...empty(), ...saved.results };
		} catch {
			// Nothing usable was saved
		}
	}
}

export const search = new SearchState();
