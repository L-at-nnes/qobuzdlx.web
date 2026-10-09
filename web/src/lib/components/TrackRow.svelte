<script lang="ts">
	import { formatDuration, withVersion } from '#lib/search/ordering.ts';
	import { search } from '#lib/search/search.svelte.ts';
	import { enqueue } from '#lib/state/enqueue.ts';
	import type { Row } from '#lib/search/selection.svelte.ts';
	import type { TrackItem } from '#lib/types.ts';
	import QualityChip from './QualityChip.svelte';
	import QueueButton from './QueueButton.svelte';
	import ResultRow from './ResultRow.svelte';

	let { track, rows }: { track: TrackItem; rows: readonly Row<number>[] } = $props();

	const title = $derived(withVersion(track.title, track.version));
	const meta = $derived(
		[track.artist, track.album?.title, formatDuration(track.duration)].filter(Boolean).join(' · ')
	);
</script>

<ResultRow
	{title}
	{meta}
	thumbnail={track.album?.thumbnail}
	explicit={track.parental_warning}
	selected={search.selectedTracks.has(track.id)}
	onselect={() => search.selectedTracks.toggle('tracks', rows, track.id)}
>
	{#snippet badges()}
		<QualityChip bitDepth={track.bit_depth} samplingRate={track.sampling_rate} />
	{/snippet}
	{#snippet actions()}
		<QueueButton
			url={track.url}
			label={title}
			onqueue={() =>
				enqueue([
					{
						url: track.url,
						label: title,
						sub_label: track.artist,
						thumbnail: track.album?.thumbnail
					}
				])}
		/>
	{/snippet}
</ResultRow>
