export type Tone = 'info' | 'error';

export interface Toast {
	id: number;
	message: string;
	tone: Tone;
}

const LIFETIME_MS = 5000;

class ToastState {
	current = $state<Toast | null>(null);
	private counter = 0;
	private timer: ReturnType<typeof setTimeout> | undefined;

	/** Shows one message at a time: the latest replaces the one before. */
	show(message: string, tone: Tone = 'info') {
		clearTimeout(this.timer);
		this.current = { id: ++this.counter, message, tone };
		this.timer = setTimeout(() => this.dismiss(), tone === 'error' ? LIFETIME_MS * 2 : LIFETIME_MS);
	}

	error(message: string) {
		this.show(message, 'error');
	}

	dismiss() {
		clearTimeout(this.timer);
		this.current = null;
	}
}

export const toast = new ToastState();
