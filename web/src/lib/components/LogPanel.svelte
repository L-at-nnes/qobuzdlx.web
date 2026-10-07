<script lang="ts">
	import { LOG_EXPORT_URL, api } from '#lib/api.ts';
	import { describeLog, t } from '#lib/i18n/index.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';

	let open = $state(false);
	let list = $state<HTMLElement>();
	let follow = true;

	// Keeps the end of the log in view while the reader has not scrolled away from it
	$effect(() => {
		void downloads.log.length;
		if (open && follow && list) list.scrollTop = list.scrollHeight;
	});

	function onscroll() {
		if (list) follow = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
	}

	async function clear() {
		await api.clearLog();
		downloads.clearLocalLog();
	}
</script>

<section class="log" aria-labelledby="log-title">
	<div class="head">
		<h2 id="log-title">
			<button type="button" class="disclosure" aria-expanded={open} onclick={() => (open = !open)}>
				<Icon name={open ? 'chevron-down' : 'chevron-right'} size={18} />
				<Icon name="log" size={17} />
				{t('log.title')}
			</button>
		</h2>
		{#if open}
			<div class="tools">
				<Button size="sm" variant="ghost" icon="download" href={LOG_EXPORT_URL}>
					{t('log.export')}
				</Button>
				<Button size="sm" variant="ghost" icon="trash" onclick={clear} disabled={downloads.log.length === 0}>
					{t('log.clear')}
				</Button>
			</div>
		{/if}
	</div>

	{#if open}
		{#if downloads.log.length === 0}
			<p class="muted empty">{t('log.empty')}</p>
		{:else}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
			<div class="lines" bind:this={list} {onscroll} role="region" tabindex="0" aria-label={t('log.title')}>
			<ol>
				{#each downloads.log as line (line.seq)}
					<li class="line {line.level}" class:header={line.event === 'item_started'}>
						{describeLog(line)}
					</li>
				{/each}
				</ol>
			</div>
		{/if}
	{/if}
</section>

<style>
	.log {
		border-top: 1px solid var(--line);
		padding-top: var(--space-3);
	}

	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	h2 {
		font-family: var(--font-ui);
		font-size: var(--text-base);
	}

	.disclosure {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-1) 0;
		border: 0;
		background: none;
		font-weight: 600;
		cursor: pointer;
	}

	.tools {
		display: flex;
		gap: var(--space-1);
	}

	.empty {
		padding: var(--space-3) 0;
		font-size: var(--text-sm);
	}

	.lines {
		max-height: 22rem;
		overflow: auto;
		margin-top: var(--space-3);
		padding: var(--space-3);
		border: 1px solid var(--line-soft);
		border-radius: var(--radius-md);
		background: var(--sunken);
		font-size: var(--text-sm);
		line-height: 1.55;
	}

	.line {
		overflow-wrap: anywhere;
		color: var(--text-muted);
	}

	.header {
		margin-top: var(--space-2);
		font-weight: 700;
		color: var(--text);
	}

	.header:first-child {
		margin-top: 0;
	}

	.line.errors {
		color: var(--danger);
	}

	.line.detailed {
		color: var(--text-faint);
	}
</style>
