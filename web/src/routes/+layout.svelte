<script lang="ts">
	import '../app.css';
	import AppShell from '#lib/components/AppShell.svelte';
	import Button from '#lib/components/Button.svelte';
	import EmptyState from '#lib/components/EmptyState.svelte';
	import SignInScreen from '#lib/components/SignInScreen.svelte';
	import ToastHost from '#lib/components/ToastHost.svelte';
	import UnlockScreen from '#lib/components/UnlockScreen.svelte';
	import { locale, t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import { theme } from '#lib/state/theme.svelte.ts';
	import { onMount } from 'svelte';

	let { children } = $props();

	onMount(() => {
		locale.start();
		theme.start();
		void app.boot();
	});
</script>

<svelte:head>
	<title>{t('app.name')}</title>
	<meta name="color-scheme" content="dark light" />
</svelte:head>

{#if app.phase === 'locked'}
	<UnlockScreen />
{:else if app.phase === 'signed-out'}
	<SignInScreen />
{:else if app.phase === 'ready'}
	<AppShell>{@render children()}</AppShell>
{:else if app.phase === 'unreachable'}
	<main class="boot">
		<EmptyState icon="warning" title={t('boot.failed')} text={t('common.offline')}>
			<Button variant="quiet" icon="retry" onclick={() => app.boot()}>{t('common.retry')}</Button>
		</EmptyState>
	</main>
{:else}
	<main class="boot" aria-busy="true" aria-label={t('common.loading')}></main>
{/if}

<ToastHost />

<style>
	.boot {
		min-height: 100dvh;
		display: grid;
		align-content: center;
	}
</style>
