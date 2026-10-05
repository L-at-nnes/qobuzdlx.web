<script lang="ts" generics="T extends string">
	import Icon, { type IconName } from './Icon.svelte';

	let {
		value = $bindable(),
		options,
		label,
		onchange
	}: {
		value: T;
		options: { value: T; label: string; icon?: IconName }[];
		/** Names the group for assistive technology. */
		label: string;
		onchange?: (value: T) => void;
	} = $props();

	const uid = $props.id();

	function choose(next: T) {
		value = next;
		onchange?.(next);
	}
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
	{#each options as option (option.value)}
		<label class:selected={value === option.value}>
			<input
				type="radio"
				name="segmented-{uid}"
				value={option.value}
				checked={value === option.value}
				onchange={() => choose(option.value)}
			/>
			{#if option.icon}<Icon name={option.icon} size={16} />{/if}
			<span>{option.label}</span>
		</label>
	{/each}
</div>

<style>
	.segmented {
		display: inline-flex;
		padding: 3px;
		gap: 2px;
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		max-width: 100%;
		overflow-x: auto;
	}

	label {
		position: relative;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		min-height: 32px;
		padding: 0 var(--space-3);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text-muted);
		cursor: pointer;
		white-space: nowrap;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			color var(--duration-fast) var(--ease-out);
	}

	label:hover {
		color: var(--text);
	}

	label.selected {
		background: var(--raised);
		color: var(--text);
		box-shadow: inset 0 0 0 1px var(--line);
	}

	input {
		position: absolute;
		opacity: 0;
		inset: 0;
		margin: 0;
		cursor: pointer;
	}

	label:has(input:focus-visible) {
		outline: 2px solid var(--accent);
		outline-offset: 1px;
	}

	@media (pointer: coarse) {
		label {
			min-height: 40px;
		}
	}
</style>
