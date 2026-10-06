<script lang="ts">
	import { formatQuality, isHiRes, shortQuality } from '#lib/search/ordering.ts';

	let { bitDepth, samplingRate }: { bitDepth: number; samplingRate: number } = $props();

	const hiRes = $derived(isHiRes(bitDepth, samplingRate));
	const short = $derived(shortQuality(bitDepth, samplingRate));
</script>

{#if short}
	<span class="chip figures" class:hi-res={hiRes} title={formatQuality(bitDepth, samplingRate)}>
		{short}
	</span>
{/if}

<style>
	.chip {
		display: inline-flex;
		align-items: center;
		height: 22px;
		padding: 0 var(--space-2);
		border-radius: 999px;
		background: var(--raised);
		color: var(--text-muted);
		font-size: var(--text-xs);
		font-weight: 600;
		white-space: nowrap;
	}

	.hi-res {
		background: var(--accent-wash);
		color: var(--text);
	}
</style>
