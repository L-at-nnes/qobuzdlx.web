<script lang="ts">
	import { api } from '#lib/api.ts';
	import { formatSpeed } from '#lib/format.ts';
	import { locale, t } from '#lib/i18n/index.svelte.ts';
	import { releaseYear } from '#lib/search/ordering.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';
	import Button from './Button.svelte';
	import Cover from './Cover.svelte';
	import ProgressBar from './ProgressBar.svelte';

	const progress = $derived(downloads.progress);
	const item = $derived(downloads.queue.find((candidate) => candidate.id === progress.item_id));
	const album = $derived(progress.album);
	/** Items that go track by track show the track in progress as part of the whole; the others report by album. */
	const byTrack = $derived(
		item !== undefined && ['album', 'track', 'playlist', 'favorite_tracks'].includes(item.kind)
	);
	/** What is finished, plus the share of each file in transfer: the bar moves while a long track comes. */
	const overall = $derived(
		byTrack
			? Math.min(
					100,
					Math.floor(
						progress.percent +
							progress.tracks.reduce((sum, track) => sum + track.percent / track.total, 0)
					)
				)
			: progress.percent
	);
	const details = $derived(
		album
			? [album.artist, releaseYear(album.release_date), album.genre, album.label, album.quality]
					.filter(Boolean)
					.join(' · ')
			: (item?.sub_label ?? '')
	);
</script>

{#if progress.running && item}
	<section class="now" aria-labelledby="now-title">
		<h2 class="eyebrow" id="now-title">{t('downloads.now')}</h2>
		<div class="body">
			<Cover src={album?.cover ?? item.thumbnail} size={88} />
			<div class="text">
				<p class="work-title big">{album?.title ?? item.label}</p>
				{#if details}<p class="meta">{details}</p>{/if}
			</div>
			<div class="actions">
				<Button size="sm" variant="quiet" icon="skip" onclick={() => api.skip()}>
					{t('downloads.skipAlbum')}
				</Button>
				<Button size="sm" variant="danger" icon="stop" onclick={() => api.cancel()}>
					{t('downloads.cancel')}
				</Button>
			</div>
		</div>

		<div class="bars">
			<div class="bar">
				<ProgressBar value={overall} label={item.label} />
				<span class="figures meta">{overall}%</span>
			</div>
			{#each progress.tracks as track (track.index)}
				<p class="track meta">
					<span class="figures">
						{t('downloads.trackOf', { index: track.index, total: track.total })}
					</span>
					<span class="name">{track.title}</span>
				</p>
				<div class="bar">
					<ProgressBar value={track.percent} label={track.title} tone="quiet" />
					<span class="figures meta">
						{track.percent}%
						{#if track.bytes_per_second}
							· {formatSpeed(track.bytes_per_second, locale.current)}
						{/if}
					</span>
				</div>
			{:else}
				<p class="track meta">{t('downloads.preparing')}</p>
			{/each}
		</div>
	</section>
{/if}

<style>
	.now {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-4);
		border: 1px solid var(--line);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}

	.eyebrow {
		font-family: var(--font-ui);
		font-size: var(--text-xs);
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	.body {
		display: flex;
		align-items: flex-start;
		gap: var(--space-4);
	}

	.text {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: var(--space-1);
	}

	.big {
		font-size: var(--text-lg);
		line-height: 1.25;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.bars {
		display: grid;
		gap: var(--space-2);
	}

	.bar {
		display: grid;
		grid-template-columns: minmax(0, 1fr) 9.5rem;
		align-items: center;
		gap: var(--space-3);
	}

	.bar .meta {
		text-align: right;
	}

	.track {
		display: flex;
		gap: var(--space-2);
		align-items: baseline;
		margin-top: var(--space-2);
	}

	.track .name {
		min-width: 0;
		color: var(--text);
		overflow-wrap: anywhere;
	}

	@media (max-width: 44rem) {
		.body {
			flex-wrap: wrap;
		}

		.actions {
			justify-content: flex-start;
			width: 100%;
		}

		.bar {
			grid-template-columns: minmax(0, 1fr) 7rem;
		}
	}
</style>
