<script lang="ts">
	import Icon from './Icon.svelte';

	let {
		src,
		size = 48,
		round = false
	}: { src: string | null | undefined; size?: number; round?: boolean } = $props();

	let failed = $state(false);

	// A new picture gets a new chance
	$effect(() => {
		void src;
		failed = false;
	});
</script>

<span class="cover" class:round style:--size="{size}px" aria-hidden="true">
	{#if src && !failed}
		<img {src} alt="" loading="lazy" decoding="async" onerror={() => (failed = true)} />
	{:else}
		<Icon name="music" size={Math.round(size * 0.42)} />
	{/if}
</span>

<style>
	.cover {
		flex: none;
		display: inline-grid;
		place-items: center;
		width: var(--size);
		height: var(--size);
		border-radius: var(--radius-sm);
		background: var(--raised);
		color: var(--text-faint);
		overflow: hidden;
	}

	.round {
		border-radius: 50%;
	}

	img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
</style>
