<script lang="ts">
	import Switch from './Switch.svelte';

	let {
		label,
		hint,
		unit,
		suggested,
		max,
		value = $bindable()
	}: {
		label: string;
		hint: string;
		/** What the number counts, said after the field. */
		unit: string;
		/** The number the condition starts with when it is switched on. */
		suggested: number;
		max: number;
		/** `0` when the condition is not used. */
		value: number;
	} = $props();

	const uid = $props.id();

	const enabled = {
		get value() {
			return value > 0;
		},
		set value(on: boolean) {
			value = on ? suggested : 0;
		}
	};

	/** A field left empty or mistyped switches the condition off rather than failing. */
	function change(event: Event) {
		const parsed = Number.parseInt((event.currentTarget as HTMLInputElement).value, 10);
		value = Number.isFinite(parsed) ? Math.min(max, Math.max(0, parsed)) : 0;
	}
</script>

<div class="condition" class:on={value > 0}>
	<Switch {label} {hint} bind:checked={enabled.value} />
	{#if value > 0}
		<label class="amount" for="condition-{uid}">
			<span class="visually-hidden">{label}</span>
			<input
				id="condition-{uid}"
				class="figures"
				type="number"
				inputmode="numeric"
				min="1"
				{max}
				step="1"
				{value}
				onchange={change}
			/>
			<span class="meta unit">{unit}</span>
		</label>
	{/if}
</div>

<style>
	.condition {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--line-soft);
		border-radius: var(--radius-md);
		background: var(--surface);
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	.condition.on {
		border-color: var(--line);
	}

	.condition :global(.switch) {
		flex: 1 1 16rem;
		min-width: 0;
	}

	.amount {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.unit {
		min-width: 3.5rem;
	}

	input {
		width: 6.5rem;
		min-height: var(--control-height);
		padding: 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		color: var(--text);
		text-align: right;
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	input:hover {
		border-color: var(--text-faint);
	}

	input:focus-visible {
		border-color: var(--accent);
	}

	@media (pointer: coarse) {
		input {
			min-height: 44px;
			font-size: 1rem;
		}
	}
</style>
