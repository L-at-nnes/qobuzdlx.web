<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { releaseYear, withVersion } from '#lib/search/ordering.ts';
	import { albumKey, search } from '#lib/search/search.svelte.ts';
	import { enqueue } from '#lib/state/enqueue.ts';
	import type { Row } from '#lib/search/selection.svelte.ts';
	import type { AlbumItem } from '#lib/types.ts';
	import ChildPanel from './ChildPanel.svelte';
	import QualityChip from './QualityChip.svelte';
	import QueueButton from './QueueButton.svelte';
	import ResultRow from './ResultRow.svelte';
	import TrackList from './TrackList.svelte';

	let {
		album,
		/** Inside an artist the artist is known: it is not said again. */
		showArtist = true,
		/** The albums of the list this one is shown in, in the order shown, and the name of that list. */
		rows,
		list
	}: { album: AlbumItem; showArtist?: boolean; rows: readonly Row<string>[]; list: string } = $props();

	const key = $derived(albumKey(album.id));
	const child = $derived(search.children[key]);
	const title = $derived(withVersion(album.title, album.version));
	const meta = $derived(
		[
			showArtist ? album.artist : '',
			releaseYear(album.release_date),
			t('row.tracks', { count: album.tracks_count }),
			album.label
		]
			.filter(Boolean)
			.join(' · ')
	);

	function queue() {
		void enqueue([
			{ url: album.url, label: title, sub_label: album.artist, thumbnail: album.thumbnail }
		]);
	}
</script>

<ResultRow
	{title}
	{meta}
	thumbnail={album.thumbnail}
	explicit={album.parental_warning}
	open={Boolean(search.open[key])}
	ontoggle={() => search.toggleAlbum(album.id)}
	openLabel={t('row.open')}
	closeLabel={t('row.close')}
	selected={search.selectedAlbums.has(album.id)}
	onselect={(range) => search.selectedAlbums.toggle(list, rows, album.id, range)}
>
	{#snippet badges()}
		<QualityChip bitDepth={album.bit_depth} samplingRate={album.sampling_rate} />
	{/snippet}
	{#snippet actions()}
		<QueueButton url={album.url} label={title} onqueue={queue} />
	{/snippet}

	<ChildPanel
		status={child?.status === 'ready' ? 'ready' : child?.status === 'failed' ? 'failed' : 'loading'}
		onretry={() => {
			search.open = { ...search.open, [key]: false };
			void search.toggleAlbum(album.id);
		}}
	>
		{#if child?.status === 'ready'}
			<TrackList tracks={child.tracks} list={key} thumbnail={album.thumbnail} />
		{/if}
	</ChildPanel>
</ResultRow>
