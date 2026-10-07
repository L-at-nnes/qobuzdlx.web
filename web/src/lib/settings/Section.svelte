<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		id,
		title,
		lead,
		children
	}: { id: string; title: string; lead?: string; children: Snippet } = $props();
</script>

<section {id} class="section" aria-labelledby="{id}-title">
	<header>
		<h2 id="{id}-title">{title}</h2>
		{#if lead}<p class="muted lead">{lead}</p>{/if}
	</header>
	<div class="body">{@render children()}</div>
</section>

<style>
	.section {
		display: grid;
		gap: var(--space-4);
		padding: var(--space-5) 0;
		border-bottom: 1px solid var(--line-soft);
		scroll-margin-top: calc(var(--bar-height) + var(--space-3));
	}

	header {
		display: grid;
		gap: var(--space-1);
	}

	.lead {
		max-width: 62ch;
		font-size: var(--text-sm);
		text-wrap: pretty;
	}

	.body {
		display: grid;
		gap: var(--space-4);
		max-width: 40rem;
	}
</style>
