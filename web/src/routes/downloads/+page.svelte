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

	/** How many rows are drawn at first, and added by each "show more": a queue can hold thousands. */
	const PAGE = 100;
	const FINISHED_AT_FIRST = 20;

	let pendingLimit = $state(PAGE);
	let finishedLimit = $state(FINISHED_AT_FIRST);

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

	/** The rows to draw: the oldest finished items and the last waiting ones are kept behind a button. */
	const shown = $derived.by(() => {
		const done = (status: string) => status !== 'pending' && status !== 'running';
		const finishedTotal = queue.filter((item) => done(item.status)).length;
		const pendingTotal = queue.filter((item) => item.status === 'pending').length;
		const hiddenFinished = Math.max(0, finishedTotal - finishedLimit);
		let finishedSeen = 0;
		let pendingSeen = 0;
		const rows = queue.filter((item) => {
			if (done(item.status)) return ++finishedSeen > hiddenFinished;
			if (item.status === 'pending') return ++pendingSeen <= pendingLimit;
			return true;
		});
		return { rows, hiddenFinished, hiddenPending: Math.max(0, pendingTotal - pendingLimit) };
	});
	const pendingIds = $derived(queue.filter((item) => item.status === 'pending').map((item) => item.id));
	const hasFinished = $derived(
		queue.some((item) => item.status !== 'pending' && item.status !== 'running')
	);
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

			<QueueEstimate />

			{#if shown.hiddenFinished > 0}
				<div class="more">
					<Button variant="ghost" size="sm" onclick={() => (finishedLimit += PAGE)}>
						{t('downloads.showEarlier', { count: shown.hiddenFinished })}
					</Button>
				</div>
			{/if}
			<ul class="queue">
				{#each shown.rows as item (item.id)}
					<QueueRow
						{item}
						first={pendingIds[0] === item.id}
						last={pendingIds.at(-1) === item.id}
					/>
				{/each}
			</ul>
			{#if shown.hiddenPending > 0}
				<div class="more">
					<Button variant="ghost" size="sm" onclick={() => (pendingLimit += PAGE)}>
						{t('downloads.showMoreWaiting', { count: shown.hiddenPending })}
					</Button>
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
