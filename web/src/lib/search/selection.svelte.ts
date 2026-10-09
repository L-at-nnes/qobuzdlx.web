import type { Wanted } from '#lib/state/enqueue.ts';
import type { AlbumItem, TrackItem } from '#lib/types.ts';
import { SvelteMap } from 'svelte/reactivity';
import { withVersion } from './ordering.ts';

/** One selectable line of a list: what it is, and what to queue if it is picked. */
export interface Row<K> {
	id: K;
	wanted: Wanted;
}

export function albumRow(album: AlbumItem): Row<string> {
	return {
		id: album.id,
		wanted: {
			url: album.url,
			label: withVersion(album.title, album.version),
			sub_label: album.artist,
			thumbnail: album.thumbnail
		}
	};
}

/** `thumbnail` stands in for the cover of the album, which the tracks of an album do not repeat. */
export function trackRow(track: TrackItem, thumbnail?: string): Row<number> {
	return {
		id: track.id,
		wanted: {
			url: track.url,
			label: withVersion(track.title, track.version),
			sub_label: track.artist,
			thumbnail: track.album?.thumbnail ?? thumbnail
		}
	};
}

/**
 * What is ticked in lists of rows. Each tick keeps what to queue, so the selection outlives the list it
 * was made in (a new search replaces the rows, not the ticks).
 */
export class Selection<K> {
	private picked = new SvelteMap<K, Wanted>();
	/** The row clicked last and the list it was in: a range is drawn from it, within that list only. */
	private anchor: { list: string; id: K } | null = null;

	get size() {
		return this.picked.size;
	}

	has(id: K) {
		return this.picked.has(id);
	}

	wanted(): Wanted[] {
		return [...this.picked.values()];
	}

	/**
	 * Ticks or unticks a row. With `range`, every row between the previous click and this one gets the
	 * state this row is going to, as shift-click does in a file explorer.
	 */
	toggle(list: string, rows: readonly Row<K>[], id: K, range = false) {
		const at = rows.findIndex((row) => row.id === id);
		if (at < 0) return;
		const on = !this.picked.has(id);
		let targets = [rows[at]];
		if (range && this.anchor?.list === list) {
			const from = rows.findIndex((row) => row.id === this.anchor?.id);
			if (from >= 0) targets = rows.slice(Math.min(from, at), Math.max(from, at) + 1);
		}
		this.set(targets, on);
		this.anchor = { list, id };
	}

	/** Ticks every row, or unticks them all. */
	setAll(rows: readonly Row<K>[], on: boolean) {
		this.set(rows, on);
	}

	clear() {
		this.picked.clear();
		this.anchor = null;
	}

	private set(rows: readonly Row<K>[], on: boolean) {
		for (const row of rows) {
			if (on) this.picked.set(row.id, row.wanted);
			else this.picked.delete(row.id);
		}
	}
}
