<script lang="ts">
	import { api } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { QueueItem } from '#lib/types.ts';
	import Button from './Button.svelte';
	import Cover from './Cover.svelte';
	import StatusBadge from './StatusBadge.svelte';

	let { item, first, last }: { item: QueueItem; first: boolean; last: boolean } = $props();

	let showFailures = $state(false);

	const finished = $derived(item.status !== 'pending' && item.status !== 'running');
	const counted = $derived(item.downloaded + item.skipped + item.failures.length > 0);
	const failed = $derived(item.failures.length > 0);
	const expired = $derived(item.status === 'failed' && item.error?.includes('session'));
</script>

<li class="item">
	<div class="line">
		<Cover src={item.thumbnail} />
		<div class="text">
			<p class="work-title">{item.label}</p>
			{#if item.sub_label}<p class="meta">{item.sub_label}</p>{/if}
			<p class="state">
				<StatusBadge status={item.status} />
				{#if counted}
					<span class="meta figures">
						{t('item.counts', {
							downloaded: item.downloaded,
							skipped: item.skipped,
							failed: item.failures.length
						})}
					</span>
				{/if}
			</p>
			{#if item.status === 'failed' && item.error}
				<p class="error">{expired ? t('item.sessionExpired') : item.error}</p>
			{/if}
		</div>

		<div class="actions">
			{#if item.status === 'pending'}
				<Button size="sm" variant="ghost" icon="arrow-up" label={t('item.moveUp')} disabled={first} onclick={() => api.move(item.id, -1)} />
				<Button size="sm" variant="ghost" icon="arrow-down" label={t('item.moveDown')} disabled={last} onclick={() => api.move(item.id, 1)} />
			{/if}
			{#if finished}
				{#if failed}
					<Button size="sm" variant="quiet" icon="retry" onclick={() => api.retryFailures(item.id)}>
						{t('item.retryFailures')}
					</Button>
				{:else}
					<Button size="sm" variant="quiet" icon="retry" onclick={() => api.retry(item.id)}>
						{t('item.retry')}
					</Button>
				{/if}
			{/if}
			{#if item.status !== 'running'}
				<Button size="sm" variant="ghost" icon="x" label={t('common.remove')} onclick={() => api.remove(item.id)} />
			{/if}
		</div>
	</div>

	{#if failed}
		<div class="failures">
			<button
				class="toggle"
				type="button"
				aria-expanded={showFailures}
				onclick={() => (showFailures = !showFailures)}
			>
				{showFailures ? t('item.hideFailures') : t('item.showFailures')}
				<span class="figures">({item.failures.length})</span>
			</button>
			{#if showFailures}
				<ul>
					{#each item.failures as failure (failure.track_id)}
						<li>
							<span class="work-title small">{failure.title}</span>
							<span class="meta">{failure.error}</span>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</li>

<style>
	.item {
		padding: var(--space-3) var(--space-2);
		border-bottom: 1px solid var(--line-soft);
	}

	.line {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
	}

	.text {
		flex: 1;
		min-width: 0;
		display: grid;
		gap: 2px;
	}

	.state {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-1) var(--space-3);
		margin-top: var(--space-1);
	}

	.error {
		margin-top: var(--space-1);
		font-size: var(--text-sm);
		color: var(--danger);
		overflow-wrap: anywhere;
	}

	.actions {
		flex: none;
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-1);
	}

	.failures {
		margin: var(--space-2) 0 0 calc(48px + var(--space-3));
	}

	.toggle {
		padding: 0;
		border: 0;
		background: none;
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 600;
		text-decoration: underline;
		text-underline-offset: 3px;
		cursor: pointer;
	}

	.toggle:hover {
		color: var(--text);
	}

	.failures ul {
		display: grid;
		gap: var(--space-2);
		margin-top: var(--space-2);
	}

	.failures li {
		display: grid;
		gap: 1px;
	}

	.small {
		font-size: var(--text-base);
	}

	@media (max-width: 44rem) {
		.line {
			flex-wrap: wrap;
		}

		.actions {
			justify-content: flex-start;
			margin-left: calc(48px + var(--space-3));
		}

		.failures {
			margin-left: 0;
		}
	}
</style>
