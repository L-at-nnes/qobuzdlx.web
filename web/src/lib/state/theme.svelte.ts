export type Theme = 'system' | 'light' | 'dark';

const STORAGE_KEY = 'qbdlx.theme';

class ThemeState {
	choice = $state<Theme>('system');

	/** Reads what was chosen on this device and applies it. */
	start() {
		try {
			const saved = localStorage.getItem(STORAGE_KEY);
			if (saved === 'light' || saved === 'dark' || saved === 'system') this.choice = saved;
		} catch {
			// Storage can be unavailable (private browsing): the system's choice stands
		}
		this.apply();
	}

	set(choice: Theme) {
		this.choice = choice;
		try {
			localStorage.setItem(STORAGE_KEY, choice);
		} catch {
			// The choice lasts until the page is closed
		}
		this.apply();
	}

	private apply() {
		const root = document.documentElement;
		if (this.choice === 'system') delete root.dataset.theme;
		else root.dataset.theme = this.choice;
	}
}

export const theme = new ThemeState();
