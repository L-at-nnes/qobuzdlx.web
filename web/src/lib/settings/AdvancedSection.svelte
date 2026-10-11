<script lang="ts">
	import SelectField from '#lib/components/SelectField.svelte';
	import Switch from '#lib/components/Switch.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { LogLevel, Settings } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	const levels: LogLevel[] = ['errors', 'normal', 'detailed'];
	const parallel = [1, 2, 3, 4, 5, 6];
</script>

<Section id="advanced" title={t('settings.advanced')}>
	<Switch
		bind:checked={draft.advanced.streamable_check}
		label={t('settings.advanced.streamable_check')}
	/>
	<Switch
		bind:checked={draft.advanced.verify_integrity}
		label={t('settings.advanced.verify_integrity')}
		hint={t('settings.advanced.verify_integrity.hint')}
	/>
	<Switch
		bind:checked={draft.advanced.fix_md5}
		label={t('settings.advanced.fix_md5')}
		hint={t('settings.advanced.fix_md5.hint')}
	/>
	<Switch
		bind:checked={draft.advanced.download_goodies}
		label={t('settings.advanced.download_goodies')}
	/>
	<Switch
		bind:checked={draft.advanced.playlist_positions}
		label={t('settings.advanced.playlist_positions')}
	/>
	<Switch bind:checked={draft.advanced.playlist_file} label={t('settings.advanced.playlist_file')} />
	<SelectField
		label={t('settings.advanced.parallel_tracks')}
		hint={t('settings.advanced.parallel_tracks.hint')}
		bind:value={draft.advanced.parallel_tracks}
		options={parallel.map((value) => ({ value, label: String(value) }))}
	/>
	<SelectField
		label={t('settings.advanced.log_detail')}
		bind:value={draft.advanced.log_detail}
		options={levels.map((value) => ({
			value,
			label: t(`log_detail.${value}` as 'log_detail.errors')
		}))}
	/>
</Section>
