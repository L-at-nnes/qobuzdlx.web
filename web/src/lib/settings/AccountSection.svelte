<script lang="ts">
	import { api } from '#lib/api.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import { toast } from '#lib/state/toast.svelte.ts';
	import type { Verdict } from '#lib/types.ts';
	import Section from './Section.svelte';

	let verdicts = $state<Verdict[] | null>(null);
	let checking = $state(false);

	async function check() {
		checking = true;
		try {
			verdicts = await api.checkConfiguration();
		} catch {
			toast.error(t('common.offline'));
		} finally {
			checking = false;
		}
	}

	async function signOut() {
		await app.signOut();
		toast.show(t('settings.account.signedOut'));
	}
</script>

<Section id="account" title={t('settings.account')}>
	{#if app.account}
		<div class="account">
			<span class="name">{app.account.display_name ?? app.account.email ?? ''}</span>
			{#if app.account.plan}
				<span class="meta">{t('settings.account.plan', { plan: app.account.plan })}</span>
			{/if}
		</div>
		<div><Button variant="quiet" icon="logout" onclick={signOut}>{t('settings.account.signOut')}</Button></div>
	{:else}
		<p class="muted">{t('settings.account.none')}</p>
	{/if}
</Section>

<Section id="diagnostics" title={t('settings.diagnostics')} lead={t('settings.diagnostics.lead')}>
	<div>
		<Button variant="quiet" icon="retry" loading={checking} onclick={check}>
			{checking ? t('settings.diagnostics.running') : t('settings.diagnostics.run')}
		</Button>
	</div>
	{#if verdicts}
		<ul class="verdicts">
			{#each verdicts as verdict (verdict.check)}
				<li class={verdict.status}>
					<Icon name={verdict.status === 'ok' ? 'check' : 'warning'} size={17} />
					<span class="words">
						<span class="what">{t(`check.${verdict.check}` as 'check.session')}</span>
						<span class="meta">
							{t(`check.${verdict.code}` as 'check.valid')}
							{#if verdict.detail}<span class="detail"> · {verdict.detail}</span>{/if}
						</span>
					</span>
				</li>
			{/each}
		</ul>
	{/if}
</Section>

<style>
	.account {
		display: grid;
		gap: 2px;
	}

	.name {
		font-family: var(--font-content);
		font-size: var(--text-md);
		font-weight: 500;
	}

	.verdicts {
		display: grid;
		gap: var(--space-3);
	}

	.verdicts li {
		display: flex;
		align-items: flex-start;
		gap: var(--space-3);
	}

	.verdicts li :global(svg) {
		margin-top: 2px;
	}

	.ok :global(svg) {
		color: var(--success);
	}

	.error :global(svg) {
		color: var(--danger);
	}

	.words {
		display: grid;
	}

	.what {
		font-weight: 600;
		font-size: var(--text-sm);
	}

	.detail {
		overflow-wrap: anywhere;
	}
</style>
