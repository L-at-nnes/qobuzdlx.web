<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { playlistKey, search } from '#lib/search/search.svelte.ts';
	import { enqueue } from '#lib/state/enqueue.ts';
	import type { PlaylistItem } from '#lib/types.ts';
	import ChildPanel from './ChildPanel.svelte';
	import QueueButton from './QueueButton.svelte';
	import ResultRow from './ResultRow.svelte';
	import TrackList from './TrackList.svelte';

	let { playlist }: { playlist: PlaylistItem } = $props();

	const key = $derived(playlistKey(playlist.id));
	const child = $derived(search.children[key]);
	const meta = $derived(
		[playlist.owner, t('row.tracks', { count: playlist.tracks_count })].filter(Boolean).join(' · ')
	);
</script>

<ResultRow
	title={playlist.name}
	{meta}
	thumbnail={playlist.image}
	open={Boolean(search.open[key])}
	ontoggle={() => search.togglePlaylist(playlist.id)}
	openLabel={t('row.open')}
	closeLabel={t('row.close')}
>
	{#snippet actions()}
		<QueueButton
			url={playlist.url}
			label={playlist.name}
			onqueue={() =>
				enqueue([
					{
						url: playlist.url,
						label: playlist.name,
						sub_label: meta,
						thumbnail: playlist.image ?? undefined
					}
				])}
		/>
	{/snippet}

	<ChildPanel
		status={child?.status === 'ready' ? 'ready' : child?.status === 'failed' ? 'failed' : 'loading'}
		onretry={() => {
			search.open = { ...search.open, [key]: false };
			void search.togglePlaylist(playlist.id);
		}}
	>
		{#if child?.status === 'ready'}
			<TrackList tracks={child.tracks} showAlbum list={key} />
		{/if}
	</ChildPanel>
</ResultRow>
