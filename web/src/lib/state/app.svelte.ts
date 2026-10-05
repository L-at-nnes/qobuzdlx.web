import { ApiError, api, onApiError } from '#lib/api.ts';
import { locale } from '#lib/i18n/index.svelte.ts';
import type { AccessStatus, Account, Settings } from '#lib/types.ts';
import { downloads } from './downloads.svelte';

/**
 * Where the application stands before anything else is shown:
 * - `booting`: finding out
 * - `unreachable`: the server does not answer; it is retried
 * - `locked`: a password is needed
 * - `signed-out`: unlocked, but not signed in to Qobuz
 * - `ready`: the application can be used
 */
export type Phase = 'booting' | 'unreachable' | 'locked' | 'signed-out' | 'ready';

const RETRY_MS = 3000;

class AppState {
	phase = $state<Phase>('booting');
	access = $state<AccessStatus | null>(null);
	account = $state<Account | null>(null);
	settings = $state<Settings | null>(null);
	/** Set when Qobuz refused a session that was working, to explain the sign-in screen. */
	sessionExpired = $state(false);

	private retry: ReturnType<typeof setTimeout> | undefined;

	constructor() {
		// What the server answers anywhere can change what is shown: the lock, a refused session
		onApiError((error) => {
			if (error.isLocked && this.phase !== 'locked') this.lockLocally();
			else if (error.isSessionExpired) this.expireSession();
		});
		downloads.onSessionExpired = () => this.expireSession();
	}

	async boot() {
		clearTimeout(this.retry);
		try {
			this.access = await api.access();
			if (this.access.protected && !this.access.unlocked) {
				this.phase = 'locked';
				return;
			}
			await this.enter();
		} catch (error) {
			if (error instanceof ApiError && error.isLocked) {
				this.phase = 'locked';
			} else {
				this.phase = 'unreachable';
				this.retry = setTimeout(() => void this.boot(), RETRY_MS);
			}
		}
	}

	/** Loads what the screens need once the application may be used. */
	async enter() {
		const [session, settings] = await Promise.all([api.session(), api.settings()]);
		this.settings = settings;
		if (settings.language) locale.set(settings.language);

		if (!session.signed_in) {
			this.account = null;
			this.phase = 'signed-out';
			return;
		}
		this.account = session.account;
		this.phase = 'ready';
		downloads.connect();
	}

	async unlock(password: string) {
		const result = await api.unlock(password);
		this.access = await api.access();
		await this.enter();
		return result.redirect;
	}

	async lock() {
		await api.lock();
		this.lockLocally();
		this.access = await api.access().catch(() => this.access);
	}

	private lockLocally() {
		downloads.disconnect();
		this.settings = null;
		this.phase = 'locked';
	}

	/** Qobuz accepted a sign-in: the application can be used. */
	async signedIn(account: Account) {
		this.account = account;
		this.sessionExpired = false;
		this.phase = 'ready';
		downloads.connect();
	}

	async signOut() {
		await api.signOut();
		this.account = null;
		this.sessionExpired = false;
		this.phase = 'signed-out';
	}

	private expireSession() {
		if (this.phase !== 'ready') return;
		this.account = null;
		this.sessionExpired = true;
		this.phase = 'signed-out';
	}

	/** Keeps what the server holds, after the settings page saved it. */
	adopt(settings: Settings) {
		this.settings = settings;
		if (settings.language) locale.set(settings.language);
	}
}

export const app = new AppState();
