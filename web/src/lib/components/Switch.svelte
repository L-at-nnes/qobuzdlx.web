<script lang="ts">
	let {
		checked = $bindable(false),
		label,
		hint,
		disabled = false
	}: { checked: boolean; label: string; hint?: string; disabled?: boolean } = $props();

	const uid = $props.id();
</script>

<div class="switch" class:disabled>
	<input id="switch-{uid}" type="checkbox" role="switch" bind:checked {disabled} aria-describedby={hint ? `switch-${uid}-hint` : undefined} />
	<label for="switch-{uid}">
		<span class="track" aria-hidden="true"><span class="thumb"></span></span>
		<span class="words">
			<span class="label">{label}</span>
			{#if hint}<span class="hint" id="switch-{uid}-hint">{hint}</span>{/if}
		</span>
	</label>
</div>

<style>
	.switch {
		position: relative;
	}

	input {
		position: absolute;
		opacity: 0;
		inset: 0 auto auto 0;
		width: 40px;
		height: 24px;
		margin: 0;
		cursor: pointer;
	}

	label {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
		cursor: pointer;
		min-height: 28px;
	}

	.track {
		flex: none;
		position: relative;
		width: 40px;
		height: 24px;
		margin-top: 1px;
		border-radius: 999px;
		background: var(--raised);
		border: 1px solid var(--line);
		transition:
			background-color var(--duration-fast) var(--ease-out),
			border-color var(--duration-fast) var(--ease-out);
	}

	.thumb {
		position: absolute;
		top: 2px;
		left: 2px;
		width: 18px;
		height: 18px;
		border-radius: 50%;
		background: var(--text-muted);
		transition:
			transform var(--duration-fast) var(--ease-out),
			background-color var(--duration-fast) var(--ease-out);
	}

	input:checked + label .track {
		background: var(--accent);
		border-color: var(--accent);
	}

	input:checked + label .thumb {
		transform: translateX(16px);
		background: var(--accent-ink);
	}

	input:focus-visible + label .track {
		outline: 2px solid var(--accent);
		outline-offset: 2px;
	}

	.words {
		display: grid;
		gap: 2px;
		padding-top: 2px;
	}

	.label {
		font-size: var(--text-base);
	}

	.hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
		text-wrap: pretty;
	}

	.disabled {
		opacity: 0.55;
	}

	.disabled label,
	.disabled input {
		cursor: not-allowed;
	}
</style>
