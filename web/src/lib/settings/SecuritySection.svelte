<script lang="ts">
	import { ApiError, api } from '#lib/api.ts';
	import Button from '#lib/components/Button.svelte';
	import Icon from '#lib/components/Icon.svelte';
	import TextField from '#lib/components/TextField.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import { toast } from '#lib/state/toast.svelte.ts';
	import Section from './Section.svelte';

	const MIN_LENGTH = 8;

	let current = $state('');
	let next = $state('');
	let again = $state('');
	let error = $state('');
	let busy = $state(false);

	const protectedNow = $derived(app.access?.protected ?? false);
	const managed = $derived(app.access?.managed ?? false);

	function explain(failure: unknown): string {
		if (failure instanceof ApiError) {
			if (failure.code === 'password_too_short') return t('settings.security.tooShort');
			if (failure.code === 'wrong_password') return t('settings.security.wrongCurrent');
			if (failure.code === 'throttled') {
				return t('unlock.throttled', {
					count: failure.retryAfter ?? 30,
					seconds: failure.retryAfter ?? 30
				});
			}
			return failure.message;
		}
		return t('boot.failed');
	}

	async function refresh() {
		app.access = await api.access();
		current = next = again = error = '';
	}

	async function save(event: SubmitEvent) {
		event.preventDefault();
		error = '';
		if (next.length < MIN_LENGTH) {
			error = t('settings.security.tooShort');
			return;
		}
		if (next !== again) {
			error = t('settings.security.mismatch');
			return;
		}
		busy = true;
		try {
			await api.setPassword(protectedNow ? current : null, next);
			await refresh();
			toast.show(t('settings.security.changed'));
		} catch (failure) {
			error = explain(failure);
		} finally {
			busy = false;
		}
	}

	async function remove() {
		error = '';
		busy = true;
		try {
			await api.removePassword(current);
			await refresh();
			toast.show(t('settings.security.removed'));
		} catch (failure) {
			error = explain(failure);
		} finally {
			busy = false;
		}
	}
</script>

<Section id="security" title={t('settings.security')} lead={t('settings.security.lead')}>
	{#if managed}
		<p class="status">
			<Icon name="lock" size={17} />
			<span>{t('settings.security.managed')}</span>
		</p>
	{:else}
		<p class="status" class:open={!protectedNow}>
			<Icon name={protectedNow ? 'lock' : 'warning'} size={17} />
			<span>{protectedNow ? t('settings.security.set') : t('settings.security.none')}</span>
		</p>

		<form onsubmit={save}>
			{#if protectedNow}
				<TextField
					label={t('settings.security.current')}
					type="password"
					autocomplete="current-password"
					bind:value={current}
				/>
			{/if}
			<TextField
				label={t('settings.security.new')}
				hint={t('settings.security.minimum')}
				type="password"
				autocomplete="new-password"
				bind:value={next}
			/>
			<TextField
				label={t('settings.security.confirm')}
				type="password"
				autocomplete="new-password"
				bind:value={again}
				{error}
			/>
			<div class="actions">
				<Button type="submit" variant="primary" loading={busy}>
					{protectedNow ? t('settings.security.changeAction') : t('settings.security.setAction')}
				</Button>
				{#if protectedNow}
					<Button variant="danger" disabled={busy || !current} onclick={remove}>
						{t('settings.security.removeAction')}
					</Button>
				{/if}
			</div>
		</form>

		{#if protectedNow}
			<div>
				<Button variant="quiet" icon="lock" onclick={() => app.lock()}>
					{t('settings.security.lockNow')}
				</Button>
			</div>
		{/if}
	{/if}
</Section>

<style>
	.status {
		display: flex;
		gap: var(--space-2);
		align-items: flex-start;
		padding: var(--space-3);
		border-radius: var(--radius-md);
		background: var(--sunken);
		font-size: var(--text-sm);
	}

	.status :global(svg) {
		margin-top: 2px;
	}

	.status.open {
		background: var(--accent-wash);
	}

	form {
		display: grid;
		gap: var(--space-4);
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
	}
</style>
