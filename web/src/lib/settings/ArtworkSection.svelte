<script lang="ts">
	import SelectField from '#lib/components/SelectField.svelte';
	import Switch from '#lib/components/Switch.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { ArtworkSize, Settings } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	const sizes = [600, 500, 400, 300, 250, 200, 150, 100, 50];

	/** The sizes to offer, with the current one when it is not a usual one. */
	function optionsFor(current: ArtworkSize) {
		const values: ArtworkSize[] = ['original', ...sizes];
		if (!values.includes(current)) values.push(current);
		return values.map((value) => ({
			value: String(value),
			label:
				value === 'original' ? t('artwork.original') : t('artwork.pixels', { pixels: value })
		}));
	}

	// The select works with text; the settings hold a number or "original"
	let embedded = $state(String(draft.artwork.embedded_size));
	let saved = $state(String(draft.artwork.saved_size));

	const parse = (value: string): ArtworkSize => (value === 'original' ? 'original' : Number(value));

	$effect(() => {
		draft.artwork.embedded_size = parse(embedded);
		draft.artwork.saved_size = parse(saved);
	});
</script>

<Section id="artwork" title={t('settings.artwork')} lead={t('artwork.hint')}>
	<SelectField
		label={t('settings.artwork.embedded')}
		bind:value={embedded}
		options={optionsFor(draft.artwork.embedded_size)}
	/>
	<Switch bind:checked={draft.artwork.save_to_disk} label={t('settings.artwork.save_to_disk')} />
	{#if draft.artwork.save_to_disk}
		<SelectField
			label={t('settings.artwork.saved')}
			bind:value={saved}
			options={optionsFor(draft.artwork.saved_size)}
		/>
	{/if}
</Section>
