<script lang="ts">
	import SelectField from '#lib/components/SelectField.svelte';
	import Switch from '#lib/components/Switch.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { Settings, TitleMarker } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	const flags = [
		'title', 'artist', 'album', 'album_artist', 'track_number', 'total_tracks', 'disc_number',
		'total_discs', 'genre', 'year', 'release_date', 'composer', 'copyright', 'label', 'isrc', 'upc',
		'release_type', 'explicit', 'cover', 'url', 'comment'
	] as const;

	const markers = [
		{ key: 'explicit_marker', label: 'settings.tags.explicit_marker' },
		{ key: 'clean_marker', label: 'settings.tags.clean_marker' }
	] as const;

	function marker(key: 'explicit_marker' | 'clean_marker'): TitleMarker {
		return draft.tags[key];
	}
</script>

<Section id="tags" title={t('settings.tags')} lead={t('settings.tags.lead')}>
	<div class="grid">
		{#each flags as flag (flag)}
			<Switch bind:checked={draft.tags[flag]} label={t(`tag.${flag}` as 'tag.title')} />
		{/each}
	</div>

	{#if draft.tags.comment}
		<TextField
			label={t('tag.comment_text')}
			hint={t('tag.comment_text.hint')}
			bind:value={draft.tags.comment_text}
			autocomplete="off"
		/>
	{/if}

	<h3>{t('settings.tags.markers')}</h3>
	{#each markers as entry (entry.key)}
		<div class="marker">
			<Switch bind:checked={draft.tags[entry.key].enabled} label={t(entry.label)} />
			{#if marker(entry.key).enabled}
				<div class="row">
					<TextField
						label={t('settings.tags.marker.text')}
						bind:value={draft.tags[entry.key].text}
						autocomplete="off"
					/>
					<SelectField
						label={t('settings.tags.marker.position')}
						bind:value={draft.tags[entry.key].position}
						options={[
							{ value: 'prefix', label: t('marker.prefix') },
							{ value: 'suffix', label: t('marker.suffix') }
						]}
					/>
				</div>
			{/if}
		</div>
	{/each}
</Section>

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(15rem, 1fr));
		gap: var(--space-3) var(--space-5);
	}

	.marker {
		display: grid;
		gap: var(--space-3);
	}

	.row {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
		gap: var(--space-3);
		padding-left: calc(40px + var(--space-3));
	}

	@media (max-width: 44rem) {
		.row {
			padding-left: 0;
		}
	}
</style>
