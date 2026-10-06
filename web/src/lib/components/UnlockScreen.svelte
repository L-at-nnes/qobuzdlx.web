<script lang="ts">
	import { goto } from '$app/navigation';
	import { ApiError } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import AuthLayout from './AuthLayout.svelte';
	import Button from './Button.svelte';
	import TextField from './TextField.svelte';

	let password = $state('');
	let error = $state('');
	let busy = $state(false);
	/** Seconds left before another attempt is accepted. */
	let waiting = $state(app.access?.locked_out_seconds ?? 0);

	$effect(() => {
		if (waiting <= 0) return;
		const timer = setInterval(() => {
			waiting = Math.max(0, waiting - 1);
			if (waiting === 0) error = '';
		}, 1000);
		return () => clearInterval(timer);
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy || waiting > 0) return;
		busy = true;
		error = '';
		try {
			const redirect = await app.unlock(password);
			password = '';
			if (redirect !== location.pathname) await goto(redirect);
		} catch (failure) {
			password = '';
			if (failure instanceof ApiError && failure.code === 'throttled') {
				waiting = failure.retryAfter ?? 30;
			} else if (failure instanceof ApiError && failure.code === 'wrong_password') {
				error = t('unlock.wrong');
			} else {
				error = failure instanceof Error ? failure.message : t('boot.failed');
			}
		} finally {
			busy = false;
		}
	}
</script>

<AuthLayout title={t('unlock.title')} lead={t('unlock.lead')}>
	<form onsubmit={submit}>
		<TextField
			label={t('unlock.password')}
			type="password"
			autocomplete="current-password"
			bind:value={password}
			error={waiting > 0 ? t('unlock.throttled', { count: waiting, seconds: waiting }) : error}
			required
		/>
		<Button type="submit" variant="primary" icon="lock" loading={busy} disabled={waiting > 0}>
			{t('unlock.submit')}
		</Button>
	</form>
</AuthLayout>

<style>
	form {
		display: grid;
		gap: var(--space-4);
	}
</style>
