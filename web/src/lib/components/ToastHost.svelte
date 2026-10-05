<script lang="ts">
	import { toast } from '#lib/state/toast.svelte.ts';
	import Icon from './Icon.svelte';
</script>

<div class="host" role="status" aria-live="polite">
	{#if toast.current}
		{#key toast.current.id}
			<div class="toast {toast.current.tone}">
				<Icon name={toast.current.tone === 'error' ? 'warning' : 'check'} size={18} />
				<span>{toast.current.message}</span>
			</div>
		{/key}
	{/if}
</div>

<style>
	.host {
		position: fixed;
		inset: auto 0 calc(var(--space-5) + env(safe-area-inset-bottom)) 0;
		display: grid;
		justify-items: center;
		padding: 0 var(--space-4);
		pointer-events: none;
		z-index: 50;
	}

	.toast {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		max-width: min(32rem, 100%);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--surface);
		box-shadow: var(--float-shadow);
		font-size: var(--text-sm);
		pointer-events: auto;
		animation: rise var(--duration-base) var(--ease-out);
	}

	.toast :global(svg) {
		margin-top: 1px;
		color: var(--success);
	}

	.error :global(svg) {
		color: var(--danger);
	}

	@keyframes rise {
		from {
			opacity: 0;
			transform: translateY(8px);
		}
	}
</style>
