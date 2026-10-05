<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import Icon from './Icon.svelte';

	interface Props extends Omit<HTMLInputAttributes, 'value'> {
		label: string;
		value: string;
		hint?: string;
		error?: string;
		/** The label is read by assistive technology but not shown (a search box says what it is). */
		hideLabel?: boolean;
	}

	let {
		label,
		value = $bindable(),
		hint,
		error,
		hideLabel = false,
		id,
		...rest
	}: Props = $props();

	const uid = $props.id();
	const fieldId = $derived(id ?? `field-${uid}`);
	const describedBy = $derived(
		[hint ? `${fieldId}-hint` : null, error ? `${fieldId}-error` : null].filter(Boolean).join(' ') ||
			undefined
	);
</script>

<div class="field">
	<label for={fieldId} class:visually-hidden={hideLabel}>{label}</label>
	<input
		id={fieldId}
		bind:value
		aria-invalid={error ? 'true' : undefined}
		aria-describedby={describedBy}
		{...rest}
	/>
	{#if hint}
		<p class="hint" id="{fieldId}-hint">{hint}</p>
	{/if}
	{#if error}
		<p class="error" id="{fieldId}-error" role="alert">
			<Icon name="warning" size={15} />
			<span>{error}</span>
		</p>
	{/if}
</div>

<style>
	.field {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
	}

	label {
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text);
	}

	input {
		width: 100%;
		min-height: var(--control-height);
		padding: 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		color: var(--text);
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	input::placeholder {
		color: var(--text-faint);
	}

	input:hover:not(:disabled) {
		border-color: var(--text-faint);
	}

	input:focus-visible {
		border-color: var(--accent);
	}

	input:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	input[aria-invalid='true'] {
		border-color: var(--danger);
	}

	.hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
		text-wrap: pretty;
	}

	.error {
		display: flex;
		align-items: flex-start;
		gap: var(--space-2);
		font-size: var(--text-sm);
		color: var(--danger);
	}

	.error :global(svg) {
		margin-top: 2px;
	}

	@media (pointer: coarse) {
		input {
			min-height: 44px;
			font-size: 1rem;
		}
	}
</style>
