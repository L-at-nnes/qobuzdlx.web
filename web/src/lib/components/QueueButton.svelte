<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';

	let {
		url,
		label,
		all = false,
		onqueue
	}: {
		url: string;
		/** What the button is for, said to assistive technology: "Download Kind of Blue". */
		label: string;
		/** Says "Download all" (an artist) instead of "Download". */
		all?: boolean;
		onqueue: () => void;
	} = $props();

	const state = $derived(downloads.stateOf(url));
</script>

<div class="queue">
	{#if state === 'done'}
		<span class="done" title={t('row.downloaded')}>
			<Icon name="check" size={16} />
			<span class="visually-hidden">{t('row.downloaded')}</span>
		</span>
	{/if}
	<Button
		variant="quiet"
		size="sm"
		icon={state === 'queued' ? 'clock' : 'download'}
		disabled={state === 'queued'}
		aria-label="{state === 'queued' ? t('row.queued') : all ? t('row.downloadAll') : t('row.download')}: {label}"
		onclick={(event: MouseEvent) => {
			event.stopPropagation();
			onqueue();
		}}
	>
		<span class="words">
			{state === 'queued' ? t('row.queued') : all ? t('row.downloadAll') : t('row.download')}
		</span>
	</Button>
</div>

<style>
	.queue {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
	}

	.done {
		display: inline-grid;
		place-items: center;
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--success-wash);
		color: var(--success);
	}

	@media (max-width: 44rem) {
		/* On a phone the button keeps its icon and gives its words to the row */
		.words {
			display: none;
		}

		.queue :global(.button) {
			width: 44px;
			padding: 0;
		}
	}
</style>
