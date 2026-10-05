<script lang="ts">
	import type { Snippet } from 'svelte';
	import QuickSettings from './QuickSettings.svelte';
	import Wordmark from './Wordmark.svelte';

	let { title, lead, children }: { title: string; lead?: string; children: Snippet } = $props();
</script>

<main class="screen" id="main">
	<div class="panel">
		<header>
			<Wordmark size="lg" />
		</header>
		<h1>{title}</h1>
		{#if lead}<p class="lead muted">{lead}</p>{/if}
		<div class="body">{@render children()}</div>
	</div>
	<footer>
		<QuickSettings />
	</footer>
</main>

<style>
	.screen {
		min-height: 100dvh;
		display: grid;
		grid-template-rows: 1fr auto;
		justify-items: center;
		padding: var(--space-5) var(--space-4) calc(var(--space-5) + env(safe-area-inset-bottom));
	}

	.panel {
		align-self: center;
		width: min(26rem, 100%);
		display: grid;
		gap: var(--space-3);
		padding: var(--space-5);
		border: 1px solid var(--line-soft);
		border-radius: var(--radius-lg);
		background: var(--surface);
	}

	header {
		margin-bottom: var(--space-2);
	}

	.lead {
		text-wrap: pretty;
		font-size: var(--text-sm);
	}

	.body {
		display: grid;
		gap: var(--space-4);
		margin-top: var(--space-2);
	}

	footer {
		margin-top: var(--space-5);
	}
</style>
