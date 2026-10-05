<script lang="ts" generics="T extends string | number">
	import Icon from './Icon.svelte';

	let {
		value = $bindable(),
		options,
		label,
		hint,
		hideLabel = false,
		compact = false,
		onchange
	}: {
		value: T;
		options: { value: T; label: string }[];
		label: string;
		hint?: string;
		hideLabel?: boolean;
		/** Sized to its content, for a toolbar. */
		compact?: boolean;
		onchange?: (value: T) => void;
	} = $props();

	const uid = $props.id();
</script>

<div class="field" class:compact>
	<label for="select-{uid}" class:visually-hidden={hideLabel}>{label}</label>
	<div class="control">
		<select
			id="select-{uid}"
			bind:value
			onchange={() => queueMicrotask(() => onchange?.(value))}
			aria-describedby={hint ? `select-${uid}-hint` : undefined}
		>
			{#each options as option (option.value)}
				<option value={option.value}>{option.label}</option>
			{/each}
		</select>
		<Icon name="chevron-down" size={16} />
	</div>
	{#if hint}<p class="hint" id="select-{uid}-hint">{hint}</p>{/if}
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
	}

	.control {
		position: relative;
	}

	select {
		appearance: none;
		width: 100%;
		min-height: var(--control-height);
		padding: 0 calc(var(--space-3) + 24px) 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		color: var(--text);
		cursor: pointer;
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	select:hover {
		border-color: var(--text-faint);
	}

	select:focus-visible {
		border-color: var(--accent);
	}

	.control :global(svg) {
		position: absolute;
		right: var(--space-3);
		top: 50%;
		transform: translateY(-50%);
		color: var(--text-muted);
		pointer-events: none;
	}

	.compact select {
		min-height: 32px;
		font-size: var(--text-sm);
	}

	.hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
		text-wrap: pretty;
	}

	@media (pointer: coarse) {
		select {
			min-height: 44px;
		}
	}
</style>
