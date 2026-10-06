<script lang="ts">
	import { page } from '$app/state';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';
	import type { Snippet } from 'svelte';
	import Button from './Button.svelte';
	import Icon, { type IconName } from './Icon.svelte';
	import Wordmark from './Wordmark.svelte';

	let { children }: { children: Snippet } = $props();

	const destinations: { href: string; label: () => string; icon: IconName }[] = [
		{ href: '/', label: () => t('nav.search'), icon: 'search' },
		{ href: '/downloads', label: () => t('nav.downloads'), icon: 'queue' },
		{ href: '/settings', label: () => t('nav.settings'), icon: 'settings' }
	];

	function current(href: string): boolean {
		return href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href);
	}
</script>

<a class="skip" href="#main">{t('nav.skip')}</a>

<div class="shell">
	<header class="top">
		<a class="brand" href="/" aria-label={t('app.name')}><Wordmark /></a>

		<nav class="tabs" aria-label={t('nav.label')}>
			{#each destinations as destination (destination.href)}
				<a href={destination.href} aria-current={current(destination.href) ? 'page' : undefined}>
					<Icon name={destination.icon} size={17} />
					<span>{destination.label()}</span>
					{#if destination.href === '/downloads' && downloads.remaining > 0}
						<span class="count figures" aria-label={t('nav.queue', { count: downloads.remaining })}>
							{downloads.remaining}
						</span>
					{/if}
				</a>
			{/each}
		</nav>

		<div class="who">
			{#if app.account?.display_name}
				<span class="name muted" title={app.account.plan ?? undefined}>{app.account.display_name}</span>
			{/if}
			{#if app.access?.protected}
				<Button variant="ghost" size="sm" icon="lock" label={t('unlock.lock')} onclick={() => app.lock()} />
			{/if}
		</div>
	</header>

	<main id="main" tabindex="-1">
		<div class="content">
			{@render children()}
		</div>
	</main>

	<nav class="bottom" aria-label={t('nav.label')}>
		{#each destinations as destination (destination.href)}
			<a href={destination.href} aria-current={current(destination.href) ? 'page' : undefined}>
				<span class="glyph">
					<Icon name={destination.icon} size={21} />
					{#if destination.href === '/downloads' && downloads.remaining > 0}
						<span class="count figures" aria-hidden="true">{downloads.remaining}</span>
					{/if}
				</span>
				<span>{destination.label()}</span>
			</a>
		{/each}
	</nav>
</div>

<style>
	.skip {
		position: absolute;
		left: var(--space-3);
		top: -4rem;
		z-index: 100;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		background: var(--accent);
		color: var(--accent-ink);
		font-weight: 600;
		text-decoration: none;
	}

	.skip:focus {
		top: var(--space-3);
	}

	.shell {
		min-height: 100dvh;
		display: grid;
		grid-template-rows: auto 1fr;
	}

	.top {
		position: sticky;
		top: 0;
		z-index: 20;
		display: flex;
		align-items: center;
		gap: var(--space-5);
		height: var(--bar-height);
		padding: 0 max(var(--space-4), env(safe-area-inset-right)) 0 max(var(--space-4), env(safe-area-inset-left));
		border-bottom: 1px solid var(--line-soft);
		background: var(--bg);
	}

	.brand {
		display: inline-flex;
		text-decoration: none;
		border-radius: var(--radius-sm);
	}

	.tabs {
		display: flex;
		gap: var(--space-1);
		margin-right: auto;
	}

	.tabs a {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		height: 36px;
		padding: 0 var(--space-3);
		border-radius: var(--radius-md);
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 600;
		text-decoration: none;
		transition:
			background-color var(--duration-fast) var(--ease-out),
			color var(--duration-fast) var(--ease-out);
	}

	.tabs a:hover {
		color: var(--text);
		background: var(--surface);
	}

	.tabs a[aria-current='page'] {
		color: var(--text);
		background: var(--raised);
	}

	.count {
		display: inline-grid;
		place-items: center;
		min-width: 20px;
		height: 20px;
		padding: 0 6px;
		border-radius: 999px;
		background: var(--accent);
		color: var(--accent-ink);
		font-size: var(--text-xs);
		font-weight: 700;
		line-height: 1;
	}

	.who {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.name {
		max-width: 12rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: var(--text-sm);
	}

	main {
		min-width: 0;
		outline: none;
	}

	.content {
		width: min(var(--content-width), 100%);
		margin: 0 auto;
		padding: var(--space-5) max(var(--space-4), env(safe-area-inset-right)) var(--space-7)
			max(var(--space-4), env(safe-area-inset-left));
	}

	.bottom {
		display: none;
	}

	@media (max-width: 44rem) {
		.tabs {
			display: none;
		}

		.top {
			justify-content: space-between;
			gap: var(--space-3);
		}

		.content {
			padding-bottom: calc(76px + var(--space-6) + env(safe-area-inset-bottom));
		}

		.bottom {
			position: fixed;
			inset: auto 0 0 0;
			z-index: 20;
			display: grid;
			grid-template-columns: repeat(3, 1fr);
			padding-bottom: env(safe-area-inset-bottom);
			border-top: 1px solid var(--line-soft);
			background: var(--bg);
		}

		.bottom a {
			display: grid;
			justify-items: center;
			gap: 2px;
			padding: var(--space-2) 0 var(--space-2);
			min-height: 60px;
			color: var(--text-muted);
			font-size: var(--text-xs);
			font-weight: 600;
			text-decoration: none;
		}

		.bottom a[aria-current='page'] {
			color: var(--text);
		}

		.glyph {
			position: relative;
			display: grid;
			place-items: center;
			width: 52px;
			height: 30px;
			border-radius: 999px;
			transition: background-color var(--duration-fast) var(--ease-out);
		}

		.bottom a[aria-current='page'] .glyph {
			background: var(--raised);
		}

		.glyph .count {
			position: absolute;
			top: -4px;
			right: 2px;
			min-width: 18px;
			height: 18px;
			font-size: 0.6875rem;
		}
	}
</style>
