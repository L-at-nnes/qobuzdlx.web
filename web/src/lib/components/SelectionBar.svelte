<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import { search } from '#lib/search/search.svelte.ts';
	import { enqueue } from '#lib/state/enqueue.ts';
	import Button from './Button.svelte';

	let busy = $state(false);

	async function download() {
		busy = true;
		try {
			if (await enqueue(search.selectedWanted())) search.clearSelection();
		} finally {
			busy = false;
		}
	}
</script>

{#if search.selectionCount > 0}
	<div class="bar" role="region" aria-label={t('selection.count', { count: search.selectionCount })}>
		<span class="count figures" aria-live="polite">
			{t('selection.count', { count: search.selectionCount })}
		</span>
		<Button variant="ghost" size="sm" onclick={() => search.clearSelection()}>
			{t('selection.clear')}
		</Button>
		<Button variant="primary" icon="download" loading={busy} onclick={download}>
			{t('selection.download')}
		</Button>
	</div>
{/if}

<style>
	.bar {
		position: fixed;
		inset: auto 0 calc(var(--space-4) + env(safe-area-inset-bottom)) 0;
		z-index: 30;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-3);
		width: fit-content;
		max-width: calc(100% - 2 * var(--space-4));
		margin: 0 auto;
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
		border: 1px solid var(--line);
		border-radius: var(--radius-lg);
		background: var(--surface);
		box-shadow: var(--float-shadow);
		animation: rise var(--duration-base) var(--ease-out);
	}

	.count {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}

	@media (max-width: 44rem) {
		.bar {
			bottom: calc(68px + var(--space-3) + env(safe-area-inset-bottom));
		}
	}
</style>
