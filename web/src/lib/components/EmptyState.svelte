<script lang="ts">
	import type { Snippet } from 'svelte';
	import Icon, { type IconName } from './Icon.svelte';

	let {
		icon,
		title,
		text,
		children
	}: { icon: IconName; title: string; text: string; children?: Snippet } = $props();
</script>

<section class="empty">
	<span class="mark"><Icon name={icon} size={22} /></span>
	<h2>{title}</h2>
	<p class="muted">{text}</p>
	{#if children}
		<div class="action">{@render children()}</div>
	{/if}
</section>

<style>
	.empty {
		display: grid;
		justify-items: center;
		gap: var(--space-3);
		padding: var(--space-7) var(--space-4);
		text-align: center;
	}

	.mark {
		display: grid;
		place-items: center;
		width: 48px;
		height: 48px;
		border-radius: 50%;
		background: var(--raised);
		color: var(--text-muted);
	}

	p {
		max-width: 44ch;
		text-wrap: pretty;
	}

	.action {
		margin-top: var(--space-2);
	}
</style>
