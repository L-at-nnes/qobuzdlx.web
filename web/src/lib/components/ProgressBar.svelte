<script lang="ts">
	let {
		value,
		label,
		tone = 'accent'
	}: { value: number; label: string; tone?: 'accent' | 'quiet' } = $props();

	const clamped = $derived(Math.max(0, Math.min(100, Math.round(value))));
</script>

<div
	class="bar {tone}"
	role="progressbar"
	aria-label={label}
	aria-valuemin="0"
	aria-valuemax="100"
	aria-valuenow={clamped}
>
	<span class="fill" style:width="{clamped}%"></span>
</div>

<style>
	.bar {
		height: 6px;
		border-radius: 999px;
		background: var(--raised);
		overflow: hidden;
	}

	.fill {
		display: block;
		height: 100%;
		border-radius: inherit;
		background: var(--accent);
		/* Only the width moves, a little, so that a jump in the numbers reads as progress */
		transition: width 400ms var(--ease-out);
	}

	.quiet .fill {
		background: var(--text-faint);
	}
</style>
