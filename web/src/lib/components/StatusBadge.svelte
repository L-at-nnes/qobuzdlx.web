<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { ItemStatus } from '#lib/types.ts';
	import Icon, { type IconName } from './Icon.svelte';

	let { status }: { status: ItemStatus } = $props();

	const icons: Record<ItemStatus, IconName> = {
		pending: 'clock',
		running: 'loader',
		completed: 'check',
		failed: 'warning',
		cancelled: 'stop'
	};
</script>

<span class="badge {status}">
	<Icon name={icons[status]} size={15} />
	<span>{t(`status.${status}` as 'status.pending')}</span>
</span>

<style>
	.badge {
		display: inline-flex;
		align-items: center;
		gap: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text-muted);
		white-space: nowrap;
	}

	.running {
		color: var(--accent);
	}

	.completed {
		color: var(--success);
	}

	.failed {
		color: var(--danger);
	}
</style>
