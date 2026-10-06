<script lang="ts">
	import { ApiError, api } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { app } from '#lib/state/app.svelte.ts';
	import AuthLayout from './AuthLayout.svelte';
	import Button from './Button.svelte';
	import Icon from './Icon.svelte';
	import TextField from './TextField.svelte';

	let withToken = $state(false);
	let email = $state('');
	let password = $state('');
	let token = $state('');
	let error = $state('');
	let busy = $state(false);

	function explain(failure: unknown): string {
		if (!(failure instanceof ApiError)) return t('signin.unavailable');
		switch (failure.code) {
			case 'wrong_credentials':
				return t('signin.wrongCredentials');
			case 'invalid_token':
				return t('signin.invalidToken');
			case 'missing_credentials':
			case 'missing_token':
				return t('signin.missing');
			case 'qobuz_identifiers_unavailable':
				return t('signin.identifiers');
			default:
				return t('signin.unavailable');
		}
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		busy = true;
		error = '';
		try {
			const account = withToken
				? await api.signInWithToken(token)
				: await api.signIn(email, password);
			password = '';
			token = '';
			await app.signedIn(account);
		} catch (failure) {
			password = '';
			error = explain(failure);
		} finally {
			busy = false;
		}
	}
</script>

<AuthLayout title={t('signin.title')} lead={t('signin.lead')}>
	{#if app.sessionExpired}
		<p class="notice" role="status">
			<Icon name="info" size={17} />
			<span>{t('signin.expired')}</span>
		</p>
	{/if}

	<form onsubmit={submit}>
		{#if withToken}
			<TextField
				label={t('signin.token')}
				hint={t('signin.tokenHelp')}
				bind:value={token}
				autocomplete="off"
				spellcheck={false}
				{error}
				required
			/>
		{:else}
			<TextField
				label={t('signin.email')}
				type="email"
				autocomplete="username"
				bind:value={email}
				required
			/>
			<TextField
				label={t('signin.password')}
				type="password"
				autocomplete="current-password"
				bind:value={password}
				{error}
				required
			/>
		{/if}
		<Button type="submit" variant="primary" loading={busy}>
			{withToken ? t('signin.tokenSubmit') : t('signin.submit')}
		</Button>
	</form>

	<Button
		variant="ghost"
		size="sm"
		onclick={() => {
			withToken = !withToken;
			error = '';
		}}
	>
		{withToken ? t('signin.withPassword') : t('signin.withToken')}
	</Button>
</AuthLayout>

<style>
	form {
		display: grid;
		gap: var(--space-4);
	}

	.notice {
		display: flex;
		gap: var(--space-2);
		align-items: flex-start;
		padding: var(--space-3);
		border-radius: var(--radius-md);
		background: var(--accent-wash);
		font-size: var(--text-sm);
	}

	.notice :global(svg) {
		margin-top: 2px;
	}
</style>
