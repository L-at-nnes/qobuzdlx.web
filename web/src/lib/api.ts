import type {
	AccessStatus,
	Account,
	AlbumItem,
	ArtistItem,
	Enqueued,
	LogLine,
	PlaylistItem,
	Progress,
	QueueItem,
	Results,
	SearchKind,
	SessionInfo,
	Settings,
	TemplatePreview,
	TrackItem,
	Verdict
} from './types';

/** What the server answers when a request goes wrong: a code to translate, a message in English. */
export class ApiError extends Error {
	constructor(
		readonly status: number,
		readonly code: string,
		message: string,
		readonly retryAfter?: number
	) {
		super(message);
		this.name = 'ApiError';
	}

	/** The application is locked: the password has to be given. */
	get isLocked(): boolean {
		return this.status === 401 && this.code === 'locked';
	}

	/** Qobuz refused the session: a new sign-in is needed. */
	get isSessionExpired(): boolean {
		return this.code === 'qobuz_session_expired' || this.code === 'qobuz_signed_out';
	}
}

type Listener = (error: ApiError) => void;
const listeners = new Set<Listener>();

/** Gets told of every failure that changes what the interface should show (locked, signed out). */
export function onApiError(listener: Listener): () => void {
	listeners.add(listener);
	return () => listeners.delete(listener);
}

/** A text is sent as it is, as a file would be; anything else as JSON. */
async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
	const isText = typeof body === 'string';
	let response: Response;
	try {
		response = await fetch(path, {
			method,
			headers:
				body === undefined
					? undefined
					: { 'content-type': isText ? 'text/plain; charset=utf-8' : 'application/json' },
			body: body === undefined ? undefined : isText ? body : JSON.stringify(body),
			credentials: 'same-origin'
		});
	} catch {
		throw new ApiError(0, 'network', 'The server could not be reached');
	}

	if (response.ok) {
		return (response.status === 204 ? undefined : await response.json()) as T;
	}

	let code = 'error';
	let message = response.statusText;
	let retryAfter: number | undefined;
	try {
		const failure = await response.json();
		code = failure.error ?? code;
		message = failure.message ?? message;
		retryAfter = failure.retry_after;
	} catch {
		// Not JSON: a proxy's page, for instance. The status says enough.
	}
	const error = new ApiError(response.status, code, message, retryAfter);
	for (const listener of listeners) listener(error);
	throw error;
}

const get = <T>(path: string) => request<T>('GET', path);
const post = <T>(path: string, body: unknown = {}) => request<T>('POST', path, body);
const put = <T>(path: string, body: unknown) => request<T>('PUT', path, body);
const del = <T>(path: string, body?: unknown) => request<T>('DELETE', path, body);

export const api = {
	// ─── Access ───
	access: () => get<AccessStatus>('/api/access'),
	unlock: (password: string, returnTo?: string) =>
		post<{ redirect: string }>('/api/access/unlock', { password, return_to: returnTo }),
	lock: () => post<void>('/api/access/lock'),
	setPassword: (current: string | null, next: string) =>
		put<void>('/api/access/password', { current, new: next }),
	removePassword: (current: string) => del<void>('/api/access/password', { current }),

	// ─── Qobuz session ───
	session: () => get<SessionInfo>('/api/session'),
	signIn: (email: string, password: string) =>
		post<Account>('/api/session/login', { email, password }),
	signInWithToken: (token: string) => post<Account>('/api/session/token', { token }),
	signOut: () => post<void>('/api/session/logout'),

	// ─── Catalogue ───
	search: (kind: SearchKind, query: string, offset = 0, limit = 50) =>
		get<Results<AlbumItem | TrackItem | ArtistItem | PlaylistItem>>(
			`/api/search/${kind}?q=${encodeURIComponent(query)}&offset=${offset}&limit=${limit}`
		),
	albumTracks: (id: string) => get<TrackItem[]>(`/api/albums/${encodeURIComponent(id)}/tracks`),
	playlistTracks: (id: number) => get<TrackItem[]>(`/api/playlists/${id}/tracks`),
	artistAlbums: (id: number) => get<AlbumItem[]>(`/api/artists/${id}/albums`),

	// ─── Downloads ───
	downloads: () =>
		get<{ queue: QueueItem[]; progress: Progress; remaining: number }>('/api/downloads'),
	enqueue: (items: { url: string; label?: string; sub_label?: string; thumbnail?: string }[]) =>
		post<Enqueued>('/api/downloads', { items }),
	/** A text with one address per line, as the file chosen by the user holds it. */
	importList: (text: string) => post<Enqueued>('/api/downloads/import', text),
	/** What each address stands for in the queue: only those that are waiting, running or done. */
	urlStates: (urls: string[]) =>
		post<Record<string, 'queued' | 'done'>>('/api/downloads/states', { urls }),
	pause: () => post<void>('/api/downloads/pause'),
	resume: () => post<void>('/api/downloads/resume'),
	cancel: () => post<void>('/api/downloads/cancel'),
	skip: () => post<void>('/api/downloads/skip'),
	clearPending: () => post<void>('/api/downloads/clear-pending'),
	clearFinished: () => post<void>('/api/downloads/clear-finished'),
	remove: (id: string) => del<void>(`/api/downloads/${id}`),
	retry: (id: string) => post<void>(`/api/downloads/${id}/retry`),
	retryFailures: (id: string) => post<void>(`/api/downloads/${id}/retry-failures`),
	move: (id: string, direction: -1 | 1) => post<void>(`/api/downloads/${id}/move`, { direction }),
	log: () => get<LogLine[]>('/api/log'),
	clearLog: () => del<void>('/api/log'),

	// ─── Settings ───
	settings: () => get<Settings>('/api/settings'),
	saveSettings: (settings: Settings) => put<Settings>('/api/settings', settings),
	previewTemplates: (settings: Settings) =>
		post<TemplatePreview>('/api/settings/preview', settings),
	checkConfiguration: () => get<Verdict[]>('/api/settings/check')
};

/** Where the log is downloaded from. */
export const LOG_EXPORT_URL = '/api/log/export';
/** The live view of the queue, the progress and the log. */
export const EVENTS_URL = '/api/downloads/events';
