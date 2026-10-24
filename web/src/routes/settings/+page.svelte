<script lang="ts">
	import { ApiError, api } from '#lib/api.ts';
	import Button from '#lib/components/Button.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import AccountSection from '#lib/settings/AccountSection.svelte';
	import AdvancedSection from '#lib/settings/AdvancedSection.svelte';
	import ArtworkSection from '#lib/settings/ArtworkSection.svelte';
	import GeneralSection from '#lib/settings/GeneralSection.svelte';
	import RateLimitSection from '#lib/settings/RateLimitSection.svelte';
	import NamingSection from '#lib/settings/NamingSection.svelte';
	import SecuritySection from '#lib/settings/SecuritySection.svelte';
	import TagsSection from '#lib/settings/TagsSection.svelte';
	import { app } from '#lib/state/app.svelte.ts';
	import { toast } from '#lib/state/toast.svelte.ts';
	import type { Settings } from '#lib/types.ts';

	/** The settings as they are being edited: what is on the server stays until they are saved. */
	let draft = $state<Settings>(structuredClone($state.snapshot(app.settings)) as Settings);
	let saved = $state(JSON.stringify($state.snapshot(app.settings)));
	let saving = $state(false);

	const dirty = $derived(JSON.stringify($state.snapshot(draft)) !== saved);

	const sections = [
		{ id: 'general', label: 'settings.general' },
		{ id: 'naming', label: 'settings.naming' },
		{ id: 'tags', label: 'settings.tags' },
		{ id: 'artwork', label: 'settings.artwork' },
		{ id: 'advanced', label: 'settings.advanced' },
		{ id: 'rate-limit', label: 'settings.rate' },
		{ id: 'security', label: 'settings.security' },
		{ id: 'account', label: 'settings.account' },
		{ id: 'diagnostics', label: 'settings.diagnostics' }
	] as const;

	async function save() {
		saving = true;
		try {
			const stored = await api.saveSettings($state.snapshot(draft) as Settings);
			app.adopt(stored);
			saved = JSON.stringify(stored);
			draft = structuredClone(stored);
			toast.show(t('settings.saved'));
		} catch (error) {
			if (error instanceof ApiError && error.code !== 'network') {
				const key = `settings.error.${error.code}` as 'settings.error.invalid_templates';
				const known = [
					'invalid_artwork_size',
					'missing_download_directory',
					'download_directory_not_absolute',
					'invalid_templates',
					'invalid_rate_limit'
				].includes(error.code);
				toast.error(known ? t(key) : `${t('settings.saveFailed')} ${error.message}`);
			} else {
				toast.error(t('settings.saveFailed'));
			}
		} finally {
			saving = false;
		}
	}

	function discard() {
		draft = structuredClone(JSON.parse(saved)) as Settings;
	}
</script>

<svelte:head>
	<title>{t('settings.title')} · {t('app.name')}</title>
</svelte:head>

<h1>{t('settings.title')}</h1>

<div class="layout">
	<nav class="sections" aria-label={t('settings.nav')}>
		{#each sections as section (section.id)}
			<a href="#{section.id}">{t(section.label)}</a>
		{/each}
	</nav>

	<div class="forms">
		<GeneralSection bind:draft />
		<NamingSection bind:draft />
		<TagsSection bind:draft />
		<ArtworkSection bind:draft />
		<AdvancedSection bind:draft />
		<RateLimitSection bind:draft />
		<SecuritySection />
		<AccountSection />
	</div>
</div>

{#if dirty}
	<div class="savebar" role="region" aria-label={t('settings.unsaved')}>
		<span class="words">{t('settings.unsaved')}</span>
		<Button variant="ghost" size="sm" onclick={discard}>{t('settings.discard')}</Button>
		<Button variant="primary" loading={saving} onclick={save}>{t('common.save')}</Button>
	</div>
{/if}

<style>
	.layout {
		display: grid;
		grid-template-columns: 11rem minmax(0, 1fr);
		gap: var(--space-6);
		margin-top: var(--space-4);
		align-items: start;
	}

	.sections {
		position: sticky;
		top: calc(var(--bar-height) + var(--space-5));
		display: grid;
		gap: 2px;
	}

	.sections a {
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 600;
		text-decoration: none;
	}

	.sections a:hover {
		color: var(--text);
		background: var(--surface);
	}

	.forms {
		min-width: 0;
	}

	.savebar {
		position: fixed;
		inset: auto 0 calc(var(--space-4) + env(safe-area-inset-bottom)) 0;
		z-index: 30;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-3);
		width: fit-content;
		max-width: calc(100% - 2 * var(--space-4));
		margin: 0 auto;
		padding: var(--space-2) var(--space-2) var(--space-2) var(--space-4);
		border: 1px solid var(--line);
		border-radius: var(--radius-lg);
		background: var(--surface);
		box-shadow: var(--float-shadow);
	}

	.words {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	@media (max-width: 56rem) {
		.layout {
			grid-template-columns: minmax(0, 1fr);
			gap: var(--space-3);
		}

		.sections {
			position: static;
			display: flex;
			gap: var(--space-1);
			overflow-x: auto;
			padding-bottom: var(--space-2);
		}

		.sections a {
			white-space: nowrap;
		}

		.savebar {
			bottom: calc(68px + var(--space-3) + env(safe-area-inset-bottom));
			flex-wrap: wrap;
		}
	}
</style>
