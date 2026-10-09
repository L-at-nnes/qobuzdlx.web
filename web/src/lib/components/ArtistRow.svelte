<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { isHiRes, sortAlbums, type Sort } from '#lib/search/ordering.ts';
	import { artistKey, search } from '#lib/search/search.svelte.ts';
	import { enqueue } from '#lib/state/enqueue.ts';
	import { albumRow } from '#lib/search/selection.svelte.ts';
	import type { ArtistItem } from '#lib/types.ts';
	import AlbumRow from './AlbumRow.svelte';
	import ChildPanel from './ChildPanel.svelte';
	import QueueButton from './QueueButton.svelte';
	import ResultRow from './ResultRow.svelte';
	import SelectField from './SelectField.svelte';
	import { locale } from '#lib/i18n/index.svelte.ts';

	let { artist }: { artist: ArtistItem } = $props();

	const key = $derived(artistKey(artist.id));
	const child = $derived(search.children[key]);
	const albums = $derived(
		child?.status === 'ready-albums'
			? sortAlbums(
					search.hiRes
						? child.albums.filter((album) => isHiRes(album.bit_depth, album.sampling_rate))
						: child.albums,
					search.artistAlbumSort,
					locale.current
				)
			: []
	);

	const albumRows = $derived(albums.map(albumRow));

	const albumSorts: Sort[] = ['relevance', 'dateDesc', 'dateAsc', 'titleAsc', 'titleDesc', 'qualityDesc', 'tracksDesc', 'tracksAsc', 'labelAsc'];
</script>

<ResultRow
	title={artist.name}
	meta={t('row.albums', { count: artist.albums_count })}
	thumbnail={artist.picture}
	round
	open={Boolean(search.open[key])}
	ontoggle={() => search.toggleArtist(artist.id)}
	openLabel={t('row.openAlbums')}
	closeLabel={t('row.closeAlbums')}
>
	{#snippet actions()}
		<QueueButton
			all
			url={artist.url}
			label={artist.name}
			onqueue={() =>
				enqueue([
					{
						url: artist.url,
						label: artist.name,
						sub_label: t('row.albums', { count: artist.albums_count }),
						thumbnail: artist.picture ?? undefined
					}
				])}
		/>
	{/snippet}

	<ChildPanel
		status={child?.status === 'ready-albums' ? 'ready' : child?.status === 'failed' ? 'failed' : 'loading'}
		onretry={() => {
			search.open = { ...search.open, [key]: false };
			void search.toggleArtist(artist.id);
		}}
	>
		<div class="toolbar">
			<SelectField
				compact
				hideLabel
				label={t('search.sort')}
				bind:value={search.artistAlbumSort}
				options={albumSorts.map((sort) => ({ value: sort, label: t(`sort.${sort}` as 'sort.relevance') }))}
			/>
		</div>
		{#if albums.length === 0}
			<p class="muted empty">{t('row.noAlbums')}</p>
		{:else}
			<ul class="albums">
				{#each albums as album (album.id)}
					<AlbumRow {album} showArtist={false} rows={albumRows} list={key} />
				{/each}
			</ul>
		{/if}
	</ChildPanel>
</ResultRow>

<style>
	.toolbar {
		display: flex;
		justify-content: flex-end;
		padding-bottom: var(--space-2);
	}

	.albums {
		border-top: 1px solid var(--line-soft);
	}

	.empty {
		padding: var(--space-3) 0;
		font-size: var(--text-sm);
	}
</style>
