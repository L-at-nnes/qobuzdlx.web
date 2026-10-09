<script lang="ts">
	import AlbumRow from '#lib/components/AlbumRow.svelte';
	import ArtistRow from '#lib/components/ArtistRow.svelte';
	import Button from '#lib/components/Button.svelte';
	import EmptyState from '#lib/components/EmptyState.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import PlaylistRow from '#lib/components/PlaylistRow.svelte';
	import SelectField from '#lib/components/SelectField.svelte';
	import SelectionBar from '#lib/components/SelectionBar.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import Switch from '#lib/components/Switch.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import TrackRow from '#lib/components/TrackRow.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.ts';
	import {
		SORTS,
		isHiRes,
		sortAlbums,
		sortArtists,
		sortPlaylists,
		sortTracks,
		type Sort
	} from '#lib/search/ordering.ts';
	import { search } from '#lib/search/search.svelte.ts';
	import { albumRow, trackRow } from '#lib/search/selection.svelte.ts';
	import type { SearchKind } from '#lib/types.ts';

	const kinds: SearchKind[] = ['albums', 'tracks', 'artists', 'playlists'];

	const found = $derived(search.results[search.kind]);
	const sort = $derived(search.sorts[search.kind]);

	/** What the page shows: the results, filtered for Hi-Res when asked, in the order chosen. */
	const albums = $derived(
		search.kind === 'albums' && search.results.albums
			? sortAlbums(
					search.hiRes
						? search.results.albums.items.filter((a) => isHiRes(a.bit_depth, a.sampling_rate))
						: search.results.albums.items,
					sort,
					locale.current
				)
			: []
	);
	const tracks = $derived(
		search.kind === 'tracks' && search.results.tracks
			? sortTracks(
					search.hiRes
						? search.results.tracks.items.filter((x) => isHiRes(x.bit_depth, x.sampling_rate))
						: search.results.tracks.items,
					sort,
					locale.current
				)
			: []
	);
	const artists = $derived(
		search.kind === 'artists' && search.results.artists
			? sortArtists(search.results.artists.items, sort, locale.current)
			: []
	);
	const playlists = $derived(
		search.kind === 'playlists' && search.results.playlists
			? sortPlaylists(search.results.playlists.items)
			: []
	);

	const shown = $derived(
		{ albums: albums.length, tracks: tracks.length, artists: artists.length, playlists: playlists.length }[
			search.kind
		]
	);
	const hasMore = $derived(found !== null && found.items.length < found.total);
	const albumRows = $derived(albums.map(albumRow));
	const trackRows = $derived(tracks.map((track) => trackRow(track)));
	const selectable = $derived(search.kind === 'albums' || search.kind === 'tracks');
	const allSelected = $derived(
		search.kind === 'albums'
			? albums.length > 0 && albums.every((a) => search.selectedAlbums.has(a.id))
			: tracks.length > 0 && tracks.every((x) => search.selectedTracks.has(x.id))
	);

	function submit(event: SubmitEvent) {
		event.preventDefault();
		void search.search();
	}

	function chooseKind(kind: SearchKind) {
		search.kind = kind;
		const query = search.query.trim();
		// A different kind of result is looked for as soon as there is something to look for
		if (query && (!search.results[kind] || search.results[kind]?.query !== query)) {
			void search.search(kind);
		}
		search.save();
	}

	function toggleAll() {
		if (search.kind === 'albums') search.selectedAlbums.setAll(albumRows, !allSelected);
		else search.selectedTracks.setAll(trackRows, !allSelected);
	}
</script>

<svelte:head>
	<title>{t('search.title')} · {t('app.name')}</title>
</svelte:head>

<h1 class="visually-hidden">{t('search.title')}</h1>

<form class="bar" onsubmit={submit} role="search">
	<TextField
		hideLabel
		label={t('search.placeholder')}
		placeholder={t('search.placeholder')}
		type="search"
		enterkeyhint="search"
		autocomplete="off"
		spellcheck={false}
		bind:value={search.query}
	/>
	<Button type="submit" variant="primary" icon="search" loading={search.searching}>
		{t('search.submit')}
	</Button>
</form>

<div class="options">
	<Segmented
		label={t('search.kind')}
		value={search.kind}
		onchange={chooseKind}
		options={kinds.map((kind) => ({ value: kind, label: t(`search.kind.${kind}` as 'search.kind.albums') }))}
	/>
	{#if search.kind === 'albums' || search.kind === 'tracks'}
		<Switch
			bind:checked={search.hiRes}
			label={t('search.hiRes')}
		/>
	{/if}
</div>

{#if search.searching && !found}
	<ul class="skeleton" aria-hidden="true">
		{#each [0, 1, 2, 3] as row (row)}
			<li><span class="swatch"></span><span class="lines"><span></span><span></span></span></li>
		{/each}
	</ul>
	<p class="visually-hidden" role="status">{t('search.searching')}</p>
{:else if search.failure}
	<div class="notice" role="alert">
		<Icon name="warning" size={18} />
		<span>{search.failure}</span>
		<Button size="sm" variant="quiet" icon="retry" onclick={() => search.search()}>
			{t('common.retry')}
		</Button>
	</div>
{:else if !search.asked}
	<EmptyState icon="search" title={t('search.empty.title')} text={t('search.empty.text')} />
{:else if found}
	{#if found.items.length === 0}
		<EmptyState icon="search" title={t('search.none', { query: found.query })} text="" />
	{:else}
		<section aria-labelledby="results-title">
			<div class="toolbar">
				<h2 id="results-title" class="count figures">
					{t('search.results', { count: shown })}
					{#if hasMore || shown !== found.items.length}
						<span class="muted"> · {t('search.resultsOf', { shown: found.items.length, total: found.total })}</span>
					{/if}
				</h2>
				<div class="tools">
					{#if selectable && shown > 0}
						<Button size="sm" variant="ghost" onclick={toggleAll}>
							{allSelected ? t('selection.deselectAll') : t('selection.selectAll')}
						</Button>
					{/if}
					{#if SORTS[search.kind].length > 1}
						<SelectField
							compact
							hideLabel
							label={t('search.sort')}
							value={sort}
							onchange={(next: Sort) => {
								search.sorts = { ...search.sorts, [search.kind]: next };
								search.save();
							}}
							options={SORTS[search.kind].map((entry) => ({
								value: entry,
								label: t(`sort.${entry}` as 'sort.relevance')
							}))}
						/>
					{/if}
				</div>
			</div>

			{#if shown === 0}
				<p class="none muted">{t('search.noneHiRes')}</p>
			{:else}
				<ul class="results" class:busy={search.searching}>
					{#each albums as album (album.id)}
						<AlbumRow {album} rows={albumRows} list="albums" />
					{/each}
					{#each tracks as track (track.id)}
						<TrackRow {track} rows={trackRows} />
					{/each}
					{#each artists as artist (artist.id)}
						<ArtistRow {artist} />
					{/each}
					{#each playlists as playlist (playlist.id)}
						<PlaylistRow {playlist} />
					{/each}
				</ul>
			{/if}

			{#if hasMore}
				<div class="more">
					<Button variant="quiet" loading={search.loadingMore} onclick={() => search.loadMore()}>
						{t('search.loadMore')}
					</Button>
				</div>
			{/if}
		</section>
	{/if}
{/if}

<SelectionBar />

<style>
	.bar {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: end;
		gap: var(--space-3);
	}

	.options {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-4);
		margin: var(--space-4) 0 var(--space-5);
	}

	.toolbar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		padding-bottom: var(--space-2);
		border-bottom: 1px solid var(--line);
	}

	.count {
		font-family: var(--font-ui);
		font-size: var(--text-sm);
		font-weight: 600;
		text-wrap: nowrap;
	}

	.tools {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.results {
		transition: opacity var(--duration-fast) var(--ease-out);
	}

	.results.busy {
		opacity: 0.55;
	}

	.more {
		display: grid;
		justify-items: center;
		padding: var(--space-5) 0;
	}

	.none {
		padding: var(--space-5) 0;
	}

	.notice {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--danger-wash);
		font-size: var(--text-sm);
	}

	.notice span {
		margin-right: auto;
	}

	.skeleton li {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-2);
		border-bottom: 1px solid var(--line-soft);
	}

	.swatch {
		width: 48px;
		height: 48px;
		border-radius: var(--radius-sm);
		background: var(--raised);
		animation: breathe 1.6s ease-in-out infinite;
	}

	.lines {
		display: grid;
		gap: var(--space-2);
		flex: 1;
	}

	.lines span {
		height: 12px;
		border-radius: 6px;
		background: var(--raised);
		animation: breathe 1.6s ease-in-out infinite;
	}

	.lines span:first-child {
		width: 55%;
		height: 14px;
	}

	.lines span:last-child {
		width: 35%;
	}

	@keyframes breathe {
		50% {
			opacity: 0.55;
		}
	}

	@media (max-width: 44rem) {
		.bar {
			grid-template-columns: 1fr;
		}
	}
</style>
