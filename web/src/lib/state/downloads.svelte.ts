import { api, EVENTS_URL } from '#lib/api.ts';
import type { LogLine, Progress, QueueItem } from '#lib/types.ts';

/** The log kept on screen. The server keeps at most as many. */
const MAX_LOG_LINES = 1200;

export type UrlState = 'queued' | 'done' | null;

const resting: Progress = { running: false, item_id: null, percent: 0, album: null, track: null };

/** The download queue as the server tells it, live. */
class DownloadsState {
	queue = $state<QueueItem[]>([]);
	progress = $state<Progress>(resting);
	log = $state<LogLine[]>([]);
	/** Whether the live view is connected. */
	connected = $state(false);

	/** Items waiting or running: what the badge of the menu counts. */
	remaining = $derived(
		this.queue.filter((item) => item.status === 'pending' || item.status === 'running').length
	);

	/** What each address stands for in the queue, so a result list can grey out and tick its buttons. */
	private states = $derived.by(() => {
		const states = new Map<string, UrlState>();
		for (const item of this.queue) {
			const url = item.url.toLowerCase();
			if (item.status === 'pending' || item.status === 'running') states.set(url, 'queued');
			else if (item.status === 'completed' && states.get(url) !== 'queued') states.set(url, 'done');
		}
		return states;
	});

	stateOf(url: string): UrlState {
		return this.states.get(url.toLowerCase()) ?? null;
	}

	private source: EventSource | undefined;
	private lastSeq = -1;

	/** Starts following the server. Calling it again does nothing while it runs. */
	connect() {
		if (this.source) return;
		const source = new EventSource(EVENTS_URL);
		this.source = source;

		source.onopen = () => {
			this.connected = true;
		};
		source.onerror = () => {
			// The browser reconnects by itself, and the server starts again from the state it has
			this.connected = false;
		};
		source.addEventListener('queue', (event) => {
			const data = JSON.parse((event as MessageEvent).data);
			this.queue = data.queue;
		});
		// One item's counts, which change with every track: the whole queue is not sent for that
		source.addEventListener('item', (event) => {
			const item = JSON.parse((event as MessageEvent).data) as QueueItem;
			const at = this.queue.findIndex((candidate) => candidate.id === item.id);
			if (at >= 0) this.queue[at] = item;
		});
		source.addEventListener('progress', (event) => {
			this.progress = JSON.parse((event as MessageEvent).data);
		});
		source.addEventListener('log', (event) => {
			this.append(JSON.parse((event as MessageEvent).data) as LogLine);
		});
		source.addEventListener('session_expired', () => {
			this.onSessionExpired?.();
		});

		void this.loadLog();
	}

	disconnect() {
		this.source?.close();
		this.source = undefined;
		this.connected = false;
	}

	/** Called when the server says Qobuz refused the session. */
	onSessionExpired: (() => void) | undefined;

	private async loadLog() {
		try {
			const lines = await api.log();
			this.log = lines.slice(-MAX_LOG_LINES);
			this.lastSeq = lines.at(-1)?.seq ?? -1;
		} catch {
			// The live view will bring the next lines anyway
		}
	}

	private append(line: LogLine) {
		// A line already in the history can come again after a reconnection
		if (line.seq <= this.lastSeq) return;
		this.lastSeq = line.seq;
		const next = [...this.log, line];
		this.log = next.length > MAX_LOG_LINES ? next.slice(-MAX_LOG_LINES) : next;
	}

	clearLocalLog() {
		this.log = [];
	}
}

export const downloads = new DownloadsState();
