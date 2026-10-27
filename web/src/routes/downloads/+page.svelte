<script lang="ts">
	import { api } from '#lib/api.ts';
	import ConfirmButton from '#lib/components/ConfirmButton.svelte';
	import EmptyState from '#lib/components/EmptyState.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import ImportList from '#lib/components/ImportList.svelte';
	import LogPanel from '#lib/components/LogPanel.svelte';
	import NowDownloading from '#lib/components/NowDownloading.svelte';
	import QueueEstimate from '#lib/components/QueueEstimate.svelte';
	import QueueRow from '#lib/components/QueueRow.svelte';
	import Button from '#lib/components/Button.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';

	/** How many rows each "show more" adds: the server is asked for them, a queue can hold thousands. */
	const PAGE = 100;
	/** Beyond this many waiting items, "show all" is not offered: the page would be drawing them all. */
	const SHOW_ALL_UP_TO = 3000;

	const queue = $derived(downloads.queue);

	/** The clock of the countdown, which runs only while the queue is holding back. */
	let tick = $state(Date.now());
	$effect(() => {
		if (!downloads.progress.wait) return;
		tick = Date.now();
		const timer = setInterval(() => (tick = Date.now()), 1000);
		return () => clearInterval(timer);
	});
	const waitEnds = $derived(
		downloads.progress.wait ? new Date(downloads.progress.wait.until * 1000) : null
	);
	const waitLeft = $derived(
		downloads.progress.wait ? Math.max(0, Math.ceil(downloads.progress.wait.until - tick / 1000)) : 0
	);

	const pendingIds = $derived(queue.filter((item) => item.status === 'pending').map((item) => item.id));
	const hiddenPending = $derived(downloads.hiddenPending);
</script>

<svelte:head>
	<title>{t('downloads.title')} · {t('app.name')}</title>
</svelte:head>

<div class="page">
	<div class="heading">
		<h1>{t('downloads.title')}</h1>
		<ImportList />
	</div>

	{#if downloads.progress.halt}
		<div class="notice" role="alert">
			<Icon name="warning" size={18} />
			<span>{t(`downloads.halted.${downloads.progress.halt}`)}</span>
			{#if downloads.progress.halt !== 'signed_out'}
				<Button size="sm" variant="quiet" icon="retry" onclick={() => api.resume()}>
					{t('downloads.resume')}
				</Button>
			{/if}
		</div>
	{/if}

	{#if downloads.progress.wait && waitEnds}
		<div class="notice calm" role="status">
			<Icon name="clock" size={18} />
			<span>
				{t(`downloads.wait.${downloads.progress.wait.reason}`, {
					time: waitEnds.toLocaleTimeString(locale.current, { timeStyle: 'short' })
				})}
				<span class="figures muted">· {waitLeft} s</span>
			</span>
		</div>
	{/if}

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
					<Button
						size="sm"
						variant="quiet"
						icon="pause"
						disabled={downloads.remaining === 0 || downloads.progress.halt !== null}
						onclick={() => api.pause()}
					>
						{t('downloads.pause')}
					</Button>
					<ConfirmButton
						label={t('downloads.clearPending')}
						question={t('downloads.confirmClearPending')}
						confirmLabel={t('downloads.confirm')}
						icon="trash"
						disabled={downloads.counts.pending === 0}
						onconfirm={() => api.clearPending()}
					/>
					<ConfirmButton
						label={t('downloads.clearFinished')}
						question={t('downloads.confirmClearFinished')}
						confirmLabel={t('downloads.confirm')}
						icon="trash"
						disabled={downloads.counts.finished === 0}
						onconfirm={() => api.clearFinished()}
					/>
				</div>
			</div>

			<QueueEstimate />

			{#if downloads.hiddenFinished > 0}
				<div class="more">
					<Button
						variant="ghost"
						size="sm"
						onclick={() => downloads.show({ finished: downloads.window.finished + PAGE })}
					>
						{t('downloads.showEarlier', { count: Math.min(PAGE, downloads.hiddenFinished) })}
					</Button>
				</div>
			{/if}
			<ul class="queue">
				{#each queue as item (item.id)}
					<QueueRow
						{item}
						first={pendingIds[0] === item.id}
						last={hiddenPending === 0 && pendingIds.at(-1) === item.id}
					/>
				{/each}
			</ul>
			{#if hiddenPending > 0}
				<div class="more">
					<Button
						variant="ghost"
						size="sm"
						onclick={() => downloads.show({ pending: downloads.window.pending + PAGE })}
					>
						{t('downloads.showMoreWaiting', { count: Math.min(PAGE, hiddenPending) })}
					</Button>
					{#if hiddenPending > PAGE && downloads.counts.pending <= SHOW_ALL_UP_TO}
						<Button
							variant="ghost"
							size="sm"
							onclick={() => downloads.show({ pending: downloads.counts.pending })}
						>
							{t('downloads.showAllWaiting', { count: downloads.counts.pending })}
						</Button>
					{/if}
				</div>
			{/if}
		</section>
	{/if}

	<LogPanel />
</div>

<style>
	.page {
		display: grid;
		gap: var(--space-5);
	}

	.notice {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--warning);
		border-radius: var(--radius-md);
		font-size: var(--text-sm);
	}

	.notice.calm {
		border-color: var(--line);
	}

	.notice span {
		flex: 1 1 18rem;
	}

	.more {
		display: flex;
		justify-content: center;
		padding: var(--space-2) 0;
	}

	.heading {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
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
