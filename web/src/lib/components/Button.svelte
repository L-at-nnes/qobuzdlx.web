<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import Icon, { type IconName } from './Icon.svelte';

	interface Props extends Omit<HTMLButtonAttributes, 'children'> {
		variant?: 'primary' | 'quiet' | 'ghost' | 'danger';
		size?: 'md' | 'sm';
		icon?: IconName;
		/** Shows a spinner and refuses clicks, keeping the width of the button. */
		loading?: boolean;
		/** A button that holds only an icon: the label then goes in `label`, for assistive technology. */
		label?: string;
		children?: Snippet;
	}

	let {
		variant = 'quiet',
		size = 'md',
		icon,
		loading = false,
		label,
		children,
		disabled,
		type = 'button',
		...rest
	}: Props = $props();
</script>

<button
	{type}
	class="button {variant} {size}"
	class:icon-only={!children}
	class:loading
	disabled={disabled || loading}
	aria-label={label}
	aria-busy={loading || undefined}
	title={label}
	{...rest}
>
	{#if loading}
		<Icon name="loader" size={size === 'sm' ? 15 : 17} />
	{:else if icon}
		<Icon name={icon} size={size === 'sm' ? 15 : 17} />
	{/if}
	{#if children}
		<span class="text">{@render children()}</span>
	{/if}
</button>

<style>
	.button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: var(--control-height);
		padding: 0 var(--space-4);
		border: 1px solid transparent;
		border-radius: var(--radius-md);
		font-size: var(--text-sm);
		font-weight: 600;
		line-height: 1.2;
		cursor: pointer;
		user-select: none;
		white-space: nowrap;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			border-color var(--duration-fast) var(--ease-out),
			color var(--duration-fast) var(--ease-out);
	}

	.sm {
		min-height: 32px;
		padding: 0 var(--space-3);
		border-radius: var(--radius-sm);
	}

	.icon-only {
		padding: 0;
		width: var(--control-height);
	}

	.sm.icon-only {
		width: 32px;
	}

	.primary {
		background: var(--accent);
		color: var(--accent-ink);
	}

	.primary:hover:not(:disabled) {
		background: var(--accent-strong);
	}

	.quiet {
		background: var(--raised);
		border-color: var(--line);
		color: var(--text);
	}

	.quiet:hover:not(:disabled) {
		border-color: var(--text-faint);
		background: color-mix(in oklch, var(--raised), var(--text) 6%);
	}

	.ghost {
		background: transparent;
		color: var(--text-muted);
	}

	.ghost:hover:not(:disabled) {
		background: var(--raised);
		color: var(--text);
	}

	.danger {
		background: transparent;
		border-color: var(--line);
		color: var(--danger);
	}

	.danger:hover:not(:disabled) {
		background: var(--danger-wash);
		border-color: var(--danger);
	}

	.button:active:not(:disabled) {
		transform: translateY(1px);
	}

	.button:disabled {
		cursor: not-allowed;
		opacity: 0.55;
	}

	.loading {
		cursor: progress;
	}

	@media (pointer: coarse) {
		.button {
			min-height: 44px;
		}

		.icon-only {
			width: 44px;
		}
	}
</style>
