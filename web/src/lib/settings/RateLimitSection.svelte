<script lang="ts">
	import BreakCondition from '#lib/components/BreakCondition.svelte';
	import DelayField from '#lib/components/DelayField.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { RateLimit, Settings } from '#lib/types.ts';
	import Section from './Section.svelte';

	let { draft = $bindable() }: { draft: Settings } = $props();

	type Profile = {
		id: 'light' | 'careful' | 'patient';
		parallel: number;
		limits: RateLimit;
	};

	/** Starting points, from the lightest to the most patient. They are advice, not measured limits. */
	const profiles: Profile[] = [
		{
			id: 'light',
			parallel: 3,
			limits: {
				between_tracks: { min_ms: 500, max_ms: 1500 },
				between_albums: { min_ms: 3000, max_ms: 8000 },
				break_after_tracks: 150,
				break_after_albums: 10,
				break_after_megabytes: 3000,
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
				break_after_tracks: 100,
				break_after_albums: 6,
				break_after_megabytes: 2000,
				break_length: { min_ms: 180_000, max_ms: 480_000 },
				cooldown_ms: 120_000
			}
		},
		{
			id: 'patient',
			parallel: 1,
			limits: {
				between_tracks: { min_ms: 2000, max_ms: 6000 },
				between_albums: { min_ms: 20_000, max_ms: 60_000 },
				break_after_tracks: 60,
				break_after_albums: 4,
				break_after_megabytes: 1000,
				break_length: { min_ms: 300_000, max_ms: 900_000 },
				cooldown_ms: 300_000
			}
		}
	];

	/** What each break condition starts with when it is switched on: the "light" values, which are the defaults. */
	const suggested = profiles[0].limits;

	const sameDelay = (a: RateLimit['break_length'], b: RateLimit['break_length']) =>
		a.min_ms === b.min_ms && a.max_ms === b.max_ms;

	function matches(profile: Profile): boolean {
		const mine = draft.rate_limit;
		const theirs = profile.limits;
		return (
			draft.advanced.parallel_tracks === profile.parallel &&
			sameDelay(mine.between_tracks, theirs.between_tracks) &&
			sameDelay(mine.between_albums, theirs.between_albums) &&
			sameDelay(mine.break_length, theirs.break_length) &&
			mine.break_after_tracks === theirs.break_after_tracks &&
			mine.break_after_albums === theirs.break_after_albums &&
			mine.break_after_megabytes === theirs.break_after_megabytes &&
			mine.cooldown_ms === theirs.cooldown_ms
		);
	}

	const custom = $derived(!profiles.some(matches));

	function apply(profile: Profile) {
		draft.rate_limit = structuredClone(profile.limits);
		draft.advanced.parallel_tracks = profile.parallel;
	}

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

	const gigabytes = (megabytes: number) => Math.round((megabytes / 1000) * 10) / 10;
	const minutes = (milliseconds: number) => Math.round((milliseconds / 60_000) * 10) / 10;

	const conditions = $derived.by(() => {
		const limits = draft.rate_limit;
		const named: string[] = [];
		if (limits.break_after_albums > 0)
			named.push(t('settings.rate.sum.albums', { count: limits.break_after_albums }));
		if (limits.break_after_tracks > 0)
			named.push(t('settings.rate.sum.tracks', { count: limits.break_after_tracks }));
		if (limits.break_after_megabytes > 0)
			named.push(
				limits.break_after_megabytes >= 1000
					? t('settings.rate.sum.gigabytes', { size: gigabytes(limits.break_after_megabytes) })
					: t('settings.rate.sum.megabytes', { size: limits.break_after_megabytes })
			);
		return named;
	});
</script>

<Section id="rate-limit" title={t('settings.rate')} lead={t('settings.rate.lead')}>
	<div class="group">
		<h3>{t('settings.rate.profiles')}</h3>
		<div class="profiles" role="group" aria-label={t('settings.rate.profiles')}>
			{#each profiles as profile (profile.id)}
				<button
					type="button"
					class="profile"
					aria-pressed={matches(profile)}
					onclick={() => apply(profile)}
				>
					<span class="name">{t(`settings.rate.profile.${profile.id}`)}</span>
					<span class="scope">{t(`settings.rate.profile.${profile.id}.scope`)}</span>
					<span class="figures meta">
						{t('settings.rate.profile.figures', {
							albums: profile.limits.break_after_albums,
							from: minutes(profile.limits.break_length.min_ms),
							to: minutes(profile.limits.break_length.max_ms)
						})}
					</span>
				</button>
			{/each}
		</div>
		<p class="muted hint">
			{custom ? t('settings.rate.custom') : t('settings.rate.profiles.hint')}
		</p>
	</div>

	<div class="group">
		<h3>{t('settings.rate.waits')}</h3>
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
	</div>

	<div class="group">
		<h3>{t('settings.rate.break')}</h3>
		<p class="muted hint">{t('settings.rate.break.lead')}</p>
		<BreakCondition
			label={t('settings.rate.break.tracks')}
			hint={t('settings.rate.break.tracks.hint')}
			unit={t('settings.rate.unit.tracks')}
			suggested={suggested.break_after_tracks}
			max={100_000}
			bind:value={draft.rate_limit.break_after_tracks}
		/>
		<BreakCondition
			label={t('settings.rate.break.albums')}
			hint={t('settings.rate.break.albums.hint')}
			unit={t('settings.rate.unit.albums')}
			suggested={suggested.break_after_albums}
			max={10_000}
			bind:value={draft.rate_limit.break_after_albums}
		/>
		<BreakCondition
			label={t('settings.rate.break.megabytes')}
			hint={t('settings.rate.break.megabytes.hint')}
			unit={t('settings.rate.unit.megabytes')}
			suggested={suggested.break_after_megabytes}
			max={10_000_000}
			bind:value={draft.rate_limit.break_after_megabytes}
		/>
		<DelayField
			label={t('settings.rate.breakLength')}
			hint={t('settings.rate.breakLength.hint')}
			unit="minutes"
			bind:value={draft.rate_limit.break_length}
		/>
		<p class="summary" role="status">
			{#if conditions.length === 0}
				{t('settings.rate.sum.none')}
			{:else}
				{t('settings.rate.sum.some', {
					from: minutes(draft.rate_limit.break_length.min_ms),
					to: minutes(draft.rate_limit.break_length.max_ms),
					conditions: conditions.join(t('settings.rate.sum.or'))
				})}
			{/if}
		</p>
	</div>

	<div class="group">
		<h3>{t('settings.rate.refused')}</h3>
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
	</div>
</Section>

<style>
	.group {
		display: grid;
		gap: var(--space-3);
	}

	h3 {
		font-family: var(--font-ui);
		font-size: var(--text-sm);
		font-weight: 600;
		letter-spacing: 0.01em;
		color: var(--text-muted);
	}

	.profiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
		gap: var(--space-3);
	}

	.profile {
		display: grid;
		align-content: start;
		gap: var(--space-1);
		padding: var(--space-3) var(--space-4);
		border: 1px solid var(--line);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		text-align: left;
		cursor: pointer;
		transition:
			border-color var(--duration-fast) var(--ease-out),
			background-color var(--duration-fast) var(--ease-out);
	}

	.profile:hover {
		border-color: var(--text-faint);
	}

	.profile[aria-pressed='true'] {
		border-color: var(--accent);
		background: var(--accent-wash);
	}

	.name {
		font-weight: 600;
	}

	.scope {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.profile .meta {
		margin-top: var(--space-1);
	}

	.hint {
		font-size: var(--text-sm);
		text-wrap: pretty;
	}

	.summary {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--raised);
		font-size: var(--text-sm);
		text-wrap: pretty;
	}
</style>
