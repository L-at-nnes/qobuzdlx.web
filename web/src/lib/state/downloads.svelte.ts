import { api, EVENTS_URL } from '#lib/api.ts';
import type { Counts, LogLine, Progress, QueueItem, QueueView, Sample, Work } from '#lib/types.ts';

/** The log kept on screen. The server keeps at most as many. */
const MAX_LOG_LINES = 1200;

export type UrlState = 'queued' | 'done' | null;

/** How many addresses are asked about at once: a page of results has a few dozen. */
const STATES_LIMIT = 500;

/** How much of the queue is asked for: a queue holds thousands of waiting items, a page shows some. */
export interface Window {
	pending: number;
	finished: number;
}

const FIRST_WINDOW: Window = { pending: 100, finished: 20 };

const resting: Progress = {
	running: false,
	item_id: null,
	percent: 0,
	album: null,
	tracks: [],
	halt: null,
	wait: null
};

/** The download queue as the server tells it, live. */
class DownloadsState {
	/** The part of the queue that was asked for: what is shown. */
	queue = $state<QueueItem[]>([]);
	progress = $state<Progress>(resting);
	log = $state<LogLine[]>([]);
	/** Whether the live view is connected. */
	connected = $state(false);

	/** Items waiting or running: what the badge of the menu counts. */
	remaining = $state(0);
	/** What the whole queue holds, and what the part leaves out. */
	counts = $state<Counts>({ pending: 0, running: 0, finished: 0 });
	hiddenPending = $state(0);
	hiddenFinished = $state(0);
	/** What the finished albums cost and what waits, for the time left. */
	history = $state<Sample[]>([]);
	work = $state<Work>({ albums: 0, tracks: 0, uncounted: 0 });
	/** How much of the queue the server is asked for. */
	window = $state<Window>({ ...FIRST_WINDOW });

	/** What each address stands for in the queue, so a result list can grey out and tick its buttons. */
	private known = $state<Record<string, UrlState>>({});
	private wanted = new Set<string>();
	private statesTimer: ReturnType<typeof setTimeout> | undefined;

	stateOf(url: string): UrlState {
		const key = url.toLowerCase();
		if (!this.wanted.has(key)) {
			this.wanted.add(key);
			this.askStates();
		}
		return this.known[key] ?? null;
	}

	/** Asks the server, once for a burst of changes, what the addresses shown stand for. */
	private askStates() {
		clearTimeout(this.statesTimer);
		this.statesTimer = setTimeout(() => void this.loadStates(), 150);
	}

	private async loadStates() {
		// The latest addresses shown are the ones that matter: older results are no longer on screen
		const urls = [...this.wanted].slice(-STATES_LIMIT);
		this.wanted = new Set(urls);
		if (urls.length === 0) return;
		try {
			const answer = await api.urlStates(urls);
			const next: Record<string, UrlState> = {};
			for (const [url, state] of Object.entries(answer)) next[url.toLowerCase()] = state;
			this.known = next;
		} catch {
			// The next change of the queue asks again
		}
	}

	/** Asks for more of the queue, or less: the live view starts again with the new part. */
	show(window: Partial<Window>) {
		this.window = { ...this.window, ...window };
		if (this.source) {
			this.disconnect();
			this.connect();
		}
	}

	private source: EventSource | undefined;
	private lastSeq = -1;

	/** Starts following the server. Calling it again does nothing while it runs. */
	connect() {
		if (this.source) return;
		const source = new EventSource(
			`${EVENTS_URL}?pending=${this.window.pending}&finished=${this.window.finished}`
		);
		this.source = source;

		source.onopen = () => {
			this.connected = true;
		};
		source.onerror = () => {
			// The browser reconnects by itself, and the server starts again from the state it has
			this.connected = false;
		};
		source.addEventListener('queue', (event) => {
			const data = JSON.parse((event as MessageEvent).data) as QueueView;
			this.queue = data.queue;
			this.remaining = data.remaining;
			this.counts = data.counts;
			this.hiddenPending = data.hidden_pending;
			this.hiddenFinished = data.hidden_finished;
			this.history = data.history;
			this.work = data.work;
			if (this.wanted.size > 0) this.askStates();
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
