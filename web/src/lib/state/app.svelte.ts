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
/** How often the session is asked about while nobody is signed in. */
const WATCH_MS = 5000;

class AppState {
	phase = $state<Phase>('booting');
	access = $state<AccessStatus | null>(null);
	account = $state<Account | null>(null);
	settings = $state<Settings | null>(null);
	/** Set when Qobuz refused a session that was working, to explain the sign-in screen. */
	sessionExpired = $state(false);
	/** A saved session waits for Qobuz to answer: the sign-in screen says it will open by itself. */
	restoring = $state(false);

	private retry: ReturnType<typeof setTimeout> | undefined;
	private watch: ReturnType<typeof setInterval> | undefined;

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
			this.restoring = session.restoring;
			this.phase = 'signed-out';
			this.watchSession();
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

	/**
	 * While nobody is signed in, the server may open a session by itself: the one it had saved, once
	 * Qobuz answers again, or one opened from another device. The page follows without a reload.
	 */
	private watchSession() {
		clearInterval(this.watch);
		this.watch = setInterval(() => void this.followSession(), WATCH_MS);
	}

	private async followSession() {
		if (this.phase !== 'signed-out') {
			clearInterval(this.watch);
			return;
		}
		try {
			const session = await api.session();
			this.restoring = session.restoring;
			if (session.signed_in && session.account && this.phase === 'signed-out') {
				await this.signedIn(session.account);
			}
		} catch {
			// The next round asks again
		}
	}

	private lockLocally() {
		clearInterval(this.watch);
		downloads.disconnect();
		this.settings = null;
		this.phase = 'locked';
	}

	/** Qobuz accepted a sign-in: the application can be used. */
	async signedIn(account: Account) {
		clearInterval(this.watch);
		this.account = account;
		this.sessionExpired = false;
		this.restoring = false;
		this.phase = 'ready';
		downloads.connect();
	}

	async signOut() {
		await api.signOut();
		this.account = null;
		this.sessionExpired = false;
		this.restoring = false;
		this.phase = 'signed-out';
		this.watchSession();
	}

	private expireSession() {
		if (this.phase !== 'ready') return;
		this.account = null;
		this.sessionExpired = true;
		this.phase = 'signed-out';
		this.watchSession();
	}

	/** Keeps what the server holds, after the settings page saved it. */
	adopt(settings: Settings) {
		this.settings = settings;
		if (settings.language) locale.set(settings.language);
	}
}

export const app = new AppState();
