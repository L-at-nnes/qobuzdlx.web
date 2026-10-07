<script lang="ts">
	import { api } from '#lib/api.ts';
	import Icon from '#lib/components/Icon.svelte';
	import Switch from '#lib/components/Switch.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { Settings, TemplateField, TemplateIssue, TemplatePreview } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	const fields: { key: TemplateField; optional?: boolean }[] = [
		{ key: 'artist' },
		{ key: 'album' },
		{ key: 'track' },
		{ key: 'compilation_track' },
		{ key: 'playlist' },
		{ key: 'disc', optional: true }
	];

	/** The variables a template can hold, by what they describe. */
	const groups = [
		{
			name: 'settings.naming.group.track',
			variables: [
				'TrackNumber', 'TrackTitle', 'TrackArtist', 'TrackComposer', 'DiscNumber', 'TrackId',
				'ISRC', 'TrackBitDepth', 'TrackSampleRate', 'TrackFormat', 'TrackFormatWithQuality',
				'TrackFormatWithHiResQuality'
			]
		},
		{
			name: 'settings.naming.group.album',
			variables: [
				'AlbumTitle', 'ArtistName', 'Year', 'ReleaseDate', 'AlbumGenre', 'AlbumComposer', 'Label',
				'Copyright', 'UPC', 'ReleaseType', 'BitDepth', 'SampleRate', 'Format', 'FormatWithQuality',
				'FormatWithHiResQuality', 'AlbumId', 'AlbumUrl'
			]
		},
		{ name: 'settings.naming.group.playlist', variables: ['PlaylistTitle', 'PlaylistId'] }
	] as const;

	let focused = $state<TemplateField>('album');
	const inputs: Partial<Record<TemplateField, HTMLInputElement>> = {};

	/** Inserts a variable where the cursor is in the field last used. */
	function insert(variable: string) {
		const input = inputs[focused];
		const text = `%${variable}%`;
		const current = draft.naming[focused];
		const start = input?.selectionStart ?? current.length;
		const end = input?.selectionEnd ?? current.length;
		draft.naming[focused] = current.slice(0, start) + text + current.slice(end);
		queueMicrotask(() => {
			input?.focus();
			input?.setSelectionRange(start + text.length, start + text.length);
		});
	}

	// What the templates give, as they are typed
	let tried = $state<TemplatePreview | null>(null);
	let timer: ReturnType<typeof setTimeout>;
	$effect(() => {
		const snapshot = $state.snapshot(draft);
		clearTimeout(timer);
		timer = setTimeout(async () => {
			try {
				tried = await api.previewTemplates(snapshot);
			} catch {
				// The preview is a help: its absence is not an error to show
			}
		}, 300);
		return () => clearTimeout(timer);
	});

	function issuesOf(field: TemplateField): TemplateIssue[] {
		return tried?.issues.filter((issue) => issue.field === field) ?? [];
	}
</script>

<Section id="naming" title={t('settings.naming')} lead={t('settings.naming.lead')}>
	{#each fields as field (field.key)}
		{@const uid = `naming-${field.key}`}
		<div class="template">
			<label for={uid}>{t(`settings.naming.${field.key}` as 'settings.naming.artist')}</label>
			<input
				id={uid}
				class="input"
				bind:value={draft.naming[field.key]}
				bind:this={inputs[field.key]}
				onfocus={() => (focused = field.key)}
				autocomplete="off"
				spellcheck="false"
				aria-describedby={field.key === 'disc' ? `${uid}-hint` : undefined}
				aria-invalid={issuesOf(field.key).length > 0 ? 'true' : undefined}
			/>
			{#if field.key === 'disc'}
				<p class="hint" id="{uid}-hint">{t('settings.naming.disc.hint')}</p>
			{/if}
			{#each issuesOf(field.key) as issue (issue.kind)}
				<p class="issue">
					<Icon name="warning" size={15} />
					<span>{t(`issue.${issue.kind}` as 'issue.empty', { detail: issue.detail ?? '' })}</span>
				</p>
			{/each}
		</div>
	{/each}

	<Switch bind:checked={draft.naming.merge_artists} label={t('settings.naming.merge_artists')} />
	<Switch
		bind:checked={draft.naming.merge_artists_in_folders}
		disabled={!draft.naming.merge_artists}
		label={t('settings.naming.merge_artists_in_folders')}
	/>

	<div class="preview" aria-live="polite">
		<h3>{t('settings.naming.preview')}</h3>
		{#if tried}
			<dl>
				<dt>{t('settings.naming.preview.album')}</dt>
				<dd>{tried.preview.album}</dd>
				<dt>{t('settings.naming.preview.compilation')}</dt>
				<dd>{tried.preview.compilation}</dd>
				<dt>{t('settings.naming.preview.playlist')}</dt>
				<dd>{tried.preview.playlist}</dd>
			</dl>
		{/if}
	</div>

	<div class="variables">
		<h3>{t('settings.naming.variables')}</h3>
		<p class="hint">{t('settings.naming.variables.hint')}</p>
		{#each groups as group (group.name)}
			<div class="group">
				<span class="group-name">{t(group.name)}</span>
				<div class="chips">
					{#each group.variables as variable (variable)}
						<button
							type="button"
							class="chip"
							onmousedown={(event) => event.preventDefault()}
							onclick={() => insert(variable)}
						>
							{variable}
						</button>
					{/each}
				</div>
			</div>
		{/each}
	</div>
</Section>

<style>
	.template {
		display: grid;
		gap: var(--space-1);
	}

	label {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.input {
		width: 100%;
		min-height: var(--control-height);
		padding: 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--sunken);
		font-family: ui-monospace, 'Cascadia Code', Consolas, monospace;
		font-size: var(--text-sm);
	}

	.input:focus-visible {
		border-color: var(--accent);
	}

	.input[aria-invalid='true'] {
		border-color: var(--warning);
	}

	.hint {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.issue {
		display: flex;
		gap: var(--space-2);
		align-items: flex-start;
		font-size: var(--text-sm);
		color: var(--warning);
	}

	.issue :global(svg) {
		margin-top: 2px;
	}

	.preview {
		display: grid;
		gap: var(--space-2);
		padding: var(--space-4);
		border-radius: var(--radius-md);
		background: var(--sunken);
		border: 1px solid var(--line-soft);
	}

	dl {
		display: grid;
		gap: var(--space-1);
	}

	dt {
		margin-top: var(--space-2);
		font-size: var(--text-xs);
		font-weight: 700;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	dd {
		margin: 0;
		font-family: ui-monospace, 'Cascadia Code', Consolas, monospace;
		font-size: var(--text-sm);
		overflow-wrap: anywhere;
	}

	.variables {
		display: grid;
		gap: var(--space-3);
	}

	.group {
		display: grid;
		gap: var(--space-2);
	}

	.group-name {
		font-size: var(--text-xs);
		font-weight: 700;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		color: var(--text-muted);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.chip {
		min-height: 30px;
		padding: 0 var(--space-3);
		border: 1px solid var(--line);
		border-radius: 999px;
		background: var(--raised);
		font-family: ui-monospace, 'Cascadia Code', Consolas, monospace;
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.chip:hover {
		border-color: var(--accent);
	}

	@media (pointer: coarse) {
		.chip {
			min-height: 40px;
		}
	}
</style>
