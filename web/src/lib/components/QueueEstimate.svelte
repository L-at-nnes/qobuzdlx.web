<script lang="ts">
	import { estimate } from '#lib/eta.ts';
	import { locale, t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import { downloads } from '#lib/state/downloads.svelte.ts';
	import Icon from './Icon.svelte';

	/** The clock, so that "done around" stays true while the queue is quiet. */
	let now = $state(Date.now());
	$effect(() => {
		const timer = setInterval(() => (now = Date.now()), 30_000);
		return () => clearInterval(timer);
	});

	const result = $derived(
		app.settings && !downloads.progress.halt
			? estimate({
					queue: downloads.queue,
					progress: downloads.progress,
					limits: app.settings.rate_limit
				})
			: null
	);

	/** A length of time as a person says it: rounded, since it is a guess. */
	function span(seconds: number): string {
		const minutes = Math.round(seconds / 60);
		if (minutes < 1) return t('eta.lessThanMinute');
		if (minutes < 60) return `${minutes} ${t('eta.unit.min')}`;
		const hours = Math.floor(minutes / 60);
		if (hours < 48) {
			const rest = Math.round((minutes % 60) / 5) * 5;
			return rest === 0 || rest === 60
				? `${rest === 60 ? hours + 1 : hours} ${t('eta.unit.h')}`
				: `${hours} ${t('eta.unit.h')} ${rest} ${t('eta.unit.min')}`;
		}
		const days = Math.floor(hours / 24);
		return `${days} ${t('eta.unit.d')} ${hours % 24} ${t('eta.unit.h')}`;
	}

	const finishes = $derived.by(() => {
		if (!result) return '';
		const date = new Date(now + result.seconds * 1000);
		const farAway = result.seconds > 18 * 3600;
		return date.toLocaleString(locale.current, {
			weekday: farAway ? 'short' : undefined,
			hour: 'numeric',
			minute: '2-digit'
		});
	});
</script>

{#if result}
	<div class="estimate" role="status">
		<p class="line">
			<Icon name="clock" size={16} />
			<span>
				{t('eta.label')}
				<strong class="figures">{t('eta.about', { time: span(result.seconds) })}</strong>
			</span>
			<span class="muted figures">
				· {t('eta.range', { low: span(result.low), high: span(result.high) })}
				· {t('eta.done', { time: finishes })}
			</span>
		</p>
		<details>
			<summary>{t('eta.how')}</summary>
			<p class="muted basis">
				{t('eta.basis', {
					albums: result.basis.albums,
					seconds: result.basis.secondsPerTrack.toFixed(1),
					tracks: Math.round(result.basis.tracksPerAlbum),
					breaks: t('eta.breaks', { count: result.basis.breaks })
				})}
				{#if result.uncounted > 0}
					{t('eta.uncounted', { count: result.uncounted })}
				{/if}
			</p>
		</details>
	</div>
{/if}

<style>
	.estimate {
		display: grid;
		gap: var(--space-1);
		padding: var(--space-3) 0 var(--space-1);
		font-size: var(--text-sm);
	}

	.line {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
	}

	.line :global(svg) {
		color: var(--text-muted);
	}

	summary {
		width: fit-content;
		color: var(--text-muted);
		cursor: pointer;
	}

	.basis {
		max-width: 62ch;
		margin-top: var(--space-2);
		text-wrap: pretty;
	}
</style>
