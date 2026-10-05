<script lang="ts">
	import { LANGUAGES, locale, t } from '#lib/i18n/index.svelte.ts';
	import { theme, type Theme } from '#lib/state/theme.svelte.ts';
	import Segmented from './Segmented.svelte';
	import SelectField from './SelectField.svelte';

	let chosen = $state<Theme>(theme.choice);
	let language = $state(locale.current);

	$effect(() => {
		chosen = theme.choice;
		language = locale.current;
	});
</script>

<div class="quick">
	<SelectField
		compact
		hideLabel
		label={t('language.label')}
		bind:value={language}
		options={LANGUAGES.map((entry) => ({ value: entry.code, label: entry.name }))}
		onchange={() => locale.set(language)}
	/>
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

<style>
	.quick {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: center;
		gap: var(--space-3);
	}
</style>
