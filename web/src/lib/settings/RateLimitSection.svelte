<script lang="ts">
	import Button from '#lib/components/Button.svelte';
	import DelayField from '#lib/components/DelayField.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { RateLimit, Settings } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	/** Starting points, from the lightest to the most patient. They are advice, not measured limits. */
	const profiles: { id: 'light' | 'careful' | 'patient'; parallel: number; limits: RateLimit }[] = [
		{
			id: 'light',
			parallel: 3,
			limits: {
				between_tracks: { min_ms: 500, max_ms: 1500 },
				between_albums: { min_ms: 3000, max_ms: 8000 },
				break_every: 0,
				break_length: { min_ms: 120_000, max_ms: 300_000 },
				cooldown_ms: 60_000
			}
		},
		{
			id: 'careful',
			parallel: 2,
			limits: {
				between_tracks: { min_ms: 1000, max_ms: 4000 },
				between_albums: { min_ms: 8000, max_ms: 20_000 },
				break_every: 25,
				break_length: { min_ms: 120_000, max_ms: 300_000 },
				cooldown_ms: 120_000
			}
		},
		{
			id: 'patient',
			parallel: 1,
			limits: {
				between_tracks: { min_ms: 2000, max_ms: 6000 },
				between_albums: { min_ms: 20_000, max_ms: 60_000 },
				break_every: 20,
				break_length: { min_ms: 300_000, max_ms: 900_000 },
				cooldown_ms: 300_000
			}
		}
	];

	function apply(profile: (typeof profiles)[number]) {
		draft.rate_limit = structuredClone(profile.limits);
		draft.advanced.parallel_tracks = profile.parallel;
	}

	/** The break and the cooldown are typed in minutes and seconds the way a person thinks of them. */
	const breakEvery = {
		get value() {
			return String(draft.rate_limit.break_every);
		},
		set value(text: string) {
			const parsed = Number.parseInt(text, 10);
			draft.rate_limit.break_every = Number.isFinite(parsed) ? Math.min(10_000, Math.max(0, parsed)) : 0;
		}
	};
	const cooldown = {
		get value() {
			return String(Math.round(draft.rate_limit.cooldown_ms / 1000));
		},
		set value(text: string) {
			const parsed = Number.parseInt(text, 10);
			draft.rate_limit.cooldown_ms = Number.isFinite(parsed)
				? Math.min(3600, Math.max(0, parsed)) * 1000
				: 0;
		}
	};
</script>

<Section id="rate-limit" title={t('settings.rate')} lead={t('settings.rate.lead')}>
	<div class="profiles">
		<p class="label">{t('settings.rate.profiles')}</p>
		<div class="buttons">
			{#each profiles as profile (profile.id)}
				<Button size="sm" variant="quiet" onclick={() => apply(profile)}>
					{t(`settings.rate.profile.${profile.id}`)}
				</Button>
			{/each}
		</div>
		<p class="muted hint">{t('settings.rate.profiles.hint')}</p>
	</div>

	<DelayField
		label={t('settings.rate.tracks')}
		hint={t('settings.rate.tracks.hint')}
		bind:value={draft.rate_limit.between_tracks}
	/>
	<DelayField
		label={t('settings.rate.albums')}
		hint={t('settings.rate.albums.hint')}
		bind:value={draft.rate_limit.between_albums}
	/>
	<TextField
		label={t('settings.rate.breakEvery')}
		hint={t('settings.rate.breakEvery.hint')}
		type="number"
		inputmode="numeric"
		min="0"
		max="10000"
		step="1"
		bind:value={breakEvery.value}
	/>
	{#if draft.rate_limit.break_every > 0}
		<DelayField
			label={t('settings.rate.breakLength')}
			hint={t('settings.rate.breakLength.hint')}
			bind:value={draft.rate_limit.break_length}
		/>
	{/if}
	<TextField
		label={t('settings.rate.cooldown')}
		hint={t('settings.rate.cooldown.hint')}
		type="number"
		inputmode="numeric"
		min="0"
		max="3600"
		step="1"
		bind:value={cooldown.value}
	/>
	<p class="muted hint">{t('settings.rate.parallel')}</p>
</Section>

<style>
	.profiles {
		display: grid;
		gap: var(--space-2);
	}

	.label {
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.buttons {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}

	.hint {
		font-size: var(--text-sm);
		text-wrap: pretty;
	}
</style>
