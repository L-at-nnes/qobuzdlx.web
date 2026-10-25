<script lang="ts">
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { Delay } from '#lib/types.ts';

	let {
		label,
		hint,
		unit = 'seconds',
		value = $bindable()
	}: { label: string; hint?: string; unit?: 'seconds' | 'minutes'; value: Delay } = $props();

	const uid = $props.id();

	/** What one unit is in milliseconds, and the longest wait the server accepts (an hour). */
	const scale = $derived(unit === 'minutes' ? 60_000 : 1000);
	const max = $derived(3_600_000 / scale);

	const shown = (milliseconds: number) => Math.round((milliseconds / scale) * 100) / 100;

	/** A field left empty or mistyped is a zero, not a failure: the server holds the bounds in order. */
	function parse(text: string): number {
		const parsed = Number.parseFloat(text.replace(',', '.'));
		return Number.isFinite(parsed) ? Math.min(max, Math.max(0, parsed)) : 0;
	}

	function change(bound: 'min_ms' | 'max_ms', event: Event) {
		value[bound] = Math.round(parse((event.currentTarget as HTMLInputElement).value) * scale);
	}

	const reversed = $derived(value.min_ms > value.max_ms);
</script>

<fieldset class="field" aria-describedby={hint ? `${uid}-hint` : undefined}>
	<legend>{label}</legend>
	<div class="bounds">
		<label>
			<span class="meta">{t('delay.from')}</span>
			<input
				type="number"
				inputmode="decimal"
				min="0"
				max={max}
				step="0.5"
				value={shown(value.min_ms)}
				aria-invalid={reversed ? 'true' : undefined}
				onchange={(event) => change('min_ms', event)}
			/>
		</label>
		<label>
			<span class="meta">{t('delay.to')}</span>
			<input
				type="number"
				inputmode="decimal"
				min="0"
				max={max}
				step="0.5"
				value={shown(value.max_ms)}
				aria-invalid={reversed ? 'true' : undefined}
				onchange={(event) => change('max_ms', event)}
			/>
		</label>
		<span class="unit meta">{t(unit === 'minutes' ? 'delay.unit.minutes' : 'delay.unit.seconds')}</span>
	</div>
	{#if reversed}
		<p class="error" role="alert">{t('delay.reversed')}</p>
	{/if}
	{#if hint}
		<p class="hint" id="{uid}-hint">{hint}</p>
	{/if}
</fieldset>

<style>
	.field {
		display: grid;
		gap: var(--space-1);
		min-width: 0;
		padding: 0;
		border: 0;
	}

	legend {
		padding: 0;
		margin-bottom: var(--space-1);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text);
	}

	.bounds {
		display: flex;
		flex-wrap: wrap;
		align-items: end;
		gap: var(--space-3);
	}

	label {
		display: grid;
		gap: var(--space-1);
	}

	input {
		width: 7rem;
		min-height: var(--control-height);
		padding: 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		color: var(--text);
		transition: border-color var(--duration-fast) var(--ease-out);
	}

	input:hover {
		border-color: var(--text-faint);
	}

	input:focus-visible {
		border-color: var(--accent);
	}

	input[aria-invalid='true'] {
		border-color: var(--danger);
	}

	.unit {
		align-self: center;
		padding-top: var(--space-4);
	}

	.hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
		text-wrap: pretty;
	}

	.error {
		font-size: var(--text-sm);
		color: var(--danger);
	}

	@media (pointer: coarse) {
		input {
			min-height: 44px;
			font-size: 1rem;
		}
	}
</style>
