<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { Snippet } from 'svelte';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';

	let {
		status,
		onretry,
		children
	}: {
		status: 'loading' | 'failed' | 'ready' | undefined;
		onretry: () => void;
		children: Snippet;
	} = $props();
</script>

{#if status === 'ready'}
	{@render children()}
{:else if status === 'failed'}
	<div class="state" role="alert">
		<Icon name="warning" size={17} />
		<span>{t('row.loadFailed')}</span>
		<Button size="sm" variant="quiet" icon="retry" onclick={onretry}>{t('common.retry')}</Button>
	</div>
{:else}
	<div class="state muted" role="status">
		<Icon name="loader" size={17} />
		<span>{t('row.loading')}</span>
	</div>
{/if}

<style>
	.state {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) 0;
		font-size: var(--text-sm);
	}
</style>
