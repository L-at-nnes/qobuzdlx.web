<script lang="ts">
	import { api } from '#lib/api.ts';
	import ConfirmButton from '#lib/components/ConfirmButton.svelte';
	import EmptyState from '#lib/components/EmptyState.svelte';
	import LogPanel from '#lib/components/LogPanel.svelte';
	import NowDownloading from '#lib/components/NowDownloading.svelte';
	import QueueRow from '#lib/components/QueueRow.svelte';
	import Button from '#lib/components/Button.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';

	const queue = $derived(downloads.queue);
	const pendingIds = $derived(queue.filter((item) => item.status === 'pending').map((item) => item.id));
	const hasFinished = $derived(
		queue.some((item) => item.status !== 'pending' && item.status !== 'running')
	);
</script>

<svelte:head>
	<title>{t('downloads.title')} · {t('app.name')}</title>
</svelte:head>

<div class="page">
	<h1>{t('downloads.title')}</h1>

	<NowDownloading />

	{#if queue.length === 0}
		<EmptyState
			icon="queue"
			title={t('downloads.empty.title')}
			text={t('downloads.empty.text')}
		>
			<Button variant="quiet" icon="search" href="/">{t('downloads.empty.action')}</Button>
		</EmptyState>
	{:else}
		<section aria-labelledby="queue-title">
			<div class="toolbar">
				<h2 id="queue-title" class="figures">
					{t('downloads.queue')}
					{#if downloads.remaining > 0}
						<span class="muted"> · {t('downloads.remaining', { count: downloads.remaining })}</span>
					{/if}
				</h2>
				<div class="tools">
					<ConfirmButton
						label={t('downloads.clearPending')}
						question={t('downloads.confirmClearPending')}
						confirmLabel={t('downloads.confirm')}
						icon="trash"
						disabled={pendingIds.length === 0}
						onconfirm={() => api.clearPending()}
					/>
					<ConfirmButton
						label={t('downloads.clearFinished')}
						question={t('downloads.confirmClearFinished')}
						confirmLabel={t('downloads.confirm')}
						icon="trash"
						disabled={!hasFinished}
						onconfirm={() => api.clearFinished()}
					/>
				</div>
			</div>

			<ul class="queue">
				{#each queue as item (item.id)}
					<QueueRow
						{item}
						first={pendingIds[0] === item.id}
						last={pendingIds.at(-1) === item.id}
					/>
				{/each}
			</ul>
		</section>
	{/if}

	<LogPanel />
</div>

<style>
	.page {
		display: grid;
		gap: var(--space-5);
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

	h2 {
		font-family: var(--font-ui);
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.tools {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
