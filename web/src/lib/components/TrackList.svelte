<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { formatDuration, trackNumber, withVersion } from '#lib/search/ordering.ts';
	import type { TrackItem } from '#lib/types.ts';
	import QualityChip from './QualityChip.svelte';

	let {
		tracks,
		/** A playlist's tracks come from other albums: each says which. */
		showAlbum = false
	}: { tracks: TrackItem[]; showAlbum?: boolean } = $props();

	const discs = $derived(new Set(tracks.map((track) => track.disc)).size);
</script>

{#if tracks.length === 0}
	<p class="empty muted">{t('row.noTracks')}</p>
{:else}
	<ol class="tracks">
		{#each tracks as track, index (track.id)}
			{#if !showAlbum && discs > 1 && (index === 0 || tracks[index - 1].disc !== track.disc)}
				<li class="disc" aria-hidden="true">{t('row.disc', { number: track.disc })}</li>
			{/if}
			<li class="track">
				<span class="number figures faint">{showAlbum ? index + 1 : trackNumber(track.number)}</span>
				<span class="text">
					<span class="work-title">{withVersion(track.title, track.version)}</span>
					<span class="meta">
						{track.artist}{#if showAlbum && track.album}<span aria-hidden="true"> · </span>{track.album.title}{/if}
					</span>
				</span>
				<span class="aside">
					<QualityChip bitDepth={track.bit_depth} samplingRate={track.sampling_rate} />
					<span class="time figures muted">{formatDuration(track.duration)}</span>
				</span>
			</li>
		{/each}
	</ol>
{/if}

<style>
	.tracks {
		display: grid;
		border-top: 1px solid var(--line-soft);
	}

	.disc {
		padding: var(--space-3) 0 var(--space-1);
		font-size: var(--text-xs);
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	.track {
		display: grid;
		grid-template-columns: 2.25rem minmax(0, 1fr) auto;
		align-items: baseline;
		gap: var(--space-3);
		padding: var(--space-2) 0;
		border-bottom: 1px solid var(--line-soft);
	}

	.track:last-child {
		border-bottom: 0;
	}

	.number {
		text-align: right;
		font-size: var(--text-sm);
	}

	.text {
		display: grid;
		gap: 1px;
		min-width: 0;
	}

	.aside {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.time {
		min-width: 3.4rem;
		text-align: right;
		font-size: var(--text-sm);
	}

	.empty {
		padding: var(--space-3) 0;
		font-size: var(--text-sm);
	}

	@media (max-width: 44rem) {
		.aside :global(.chip) {
			display: none;
		}
	}
</style>
