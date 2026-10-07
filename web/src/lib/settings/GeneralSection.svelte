<script lang="ts">
	import SelectField from '#lib/components/SelectField.svelte';
	import Segmented from '#lib/components/Segmented.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { LANGUAGES, locale, t } from '#lib/i18n/index.svelte.ts';
	import { theme, type Theme } from '#lib/state/theme.svelte.ts';
	import type { Duplicates, Language, Quality, Settings } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	let language = $state<Language>(draft.language ?? locale.current);
	let chosen = $state<Theme>(theme.choice);

	$effect(() => {
		if (language !== (draft.language ?? locale.current)) draft.language = language;
	});

	const qualities: Quality[] = ['flac24_max', 'flac24', 'flac16', 'mp3'];
	const duplicates: Duplicates[] = ['skip', 'rename', 'overwrite'];
</script>

<Section id="general" title={t('settings.general')}>
	<SelectField
		label={t('settings.general.language')}
		hint={t('settings.general.language.hint')}
		bind:value={language}
		options={LANGUAGES.map((entry) => ({ value: entry.code, label: entry.name }))}
	/>

	<div class="appearance">
		<span class="label">{t('theme.label')}</span>
		<Segmented
			label={t('theme.label')}
			bind:value={chosen}
			onchange={(value) => theme.set(value)}
			options={[
				{ value: 'system', label: t('theme.system'), icon: 'monitor' },
				{ value: 'light', label: t('theme.light'), icon: 'sun' },
				{ value: 'dark', label: t('theme.dark'), icon: 'moon' }
			]}
		/>
	</div>

	<TextField
		label={t('settings.general.directory')}
		hint={t('settings.general.directory.hint')}
		bind:value={draft.download.directory}
		autocomplete="off"
		spellcheck={false}
	/>

	<SelectField
		label={t('settings.general.quality')}
		hint={t('settings.general.quality.hint')}
		bind:value={draft.download.quality}
		options={qualities.map((value) => ({ value, label: t(`quality.${value}` as 'quality.mp3') }))}
	/>

	<SelectField
		label={t('settings.general.duplicates')}
		bind:value={draft.advanced.duplicates}
		options={duplicates.map((value) => ({
			value,
			label: t(`duplicates.${value}` as 'duplicates.skip')
		}))}
	/>
</Section>

<style>
	.appearance {
		display: grid;
		gap: var(--space-1);
		justify-items: start;
	}

	.label {
		font-size: var(--text-sm);
		font-weight: 600;
	}
</style>
