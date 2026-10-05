<script lang="ts" module>
	/** Outlined icons on a 24 px grid, drawn with a 1.75 px round stroke. */
	const shapes = {
		search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
		download: '<path d="M12 4v11"/><path d="m7 11 5 5 5-5"/><path d="M5 20h14"/>',
		queue:
			'<path d="M9 6h11M9 12h11M9 18h11"/><circle cx="4.5" cy="6" r="1"/><circle cx="4.5" cy="12" r="1"/><circle cx="4.5" cy="18" r="1"/>',
		settings:
			'<path d="M4 7h9M17 7h3M4 17h3M11 17h9"/><circle cx="15" cy="7" r="2"/><circle cx="9" cy="17" r="2"/>',
		check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
		x: '<path d="M6 6l12 12M18 6 6 18"/>',
		'chevron-down': '<path d="m6 9 6 6 6-6"/>',
		'chevron-right': '<path d="m9 6 6 6-6 6"/>',
		'arrow-up': '<path d="M12 19V5"/><path d="m6 11 6-6 6 6"/>',
		'arrow-down': '<path d="M12 5v14"/><path d="m6 13 6 6 6-6"/>',
		retry: '<path d="M4 12a8 8 0 1 0 2.6-5.9"/><path d="M4 4v4.5h4.5"/>',
		skip: '<path d="m5 5 9 7-9 7z"/><path d="M18 5v14"/>',
		trash: '<path d="M5 7h14"/><path d="M10 7V5h4v2"/><path d="m7 7 1 12h8l1-12"/>',
		lock: '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
		sun: '<circle cx="12" cy="12" r="4"/><path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6 7 7M17 17l1.4 1.4M5.6 18.4 7 17M17 7l1.4-1.4"/>',
		moon: '<path d="M20 14.5A8 8 0 0 1 9.5 4 8 8 0 1 0 20 14.5z"/>',
		monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
		globe:
			'<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>',
		warning:
			'<path d="M12 4 2.8 20h18.4z"/><path d="M12 10v5"/><circle cx="12" cy="17.6" r=".7" fill="currentColor"/>',
		info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><circle cx="12" cy="7.7" r=".7" fill="currentColor"/>',
		music:
			'<path d="M9 18V6l10-2v12"/><circle cx="6.5" cy="18" r="2.5"/><circle cx="16.5" cy="16" r="2.5"/>',
		disc: '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="2.5"/>',
		logout:
			'<path d="M10 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h4"/><path d="m15 8 4 4-4 4M19 12H9"/>',
		key: '<circle cx="8" cy="15" r="4"/><path d="m11 12 8-8M16 7l3 3"/>',
		folder:
			'<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
		clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
		stop: '<circle cx="12" cy="12" r="9"/><path d="m6 6 12 12"/>',
		log: '<path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5M10 13h6M10 17h6"/>',
		loader: '<path d="M12 3a9 9 0 1 0 9 9"/>'
	} as const;

	export type IconName = keyof typeof shapes;
</script>

<script lang="ts">
	let { name, size = 18 }: { name: IconName; size?: number } = $props();
</script>

<svg
	class="icon"
	class:spin={name === 'loader'}
	width={size}
	height={size}
	viewBox="0 0 24 24"
	fill="none"
	stroke="currentColor"
	stroke-width="1.75"
	stroke-linecap="round"
	stroke-linejoin="round"
	aria-hidden="true"
>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html shapes[name]}
</svg>

<style>
	.icon {
		flex: none;
	}

	.spin {
		animation: turn 0.9s linear infinite;
	}

	@keyframes turn {
		to {
			transform: rotate(360deg);
		}
	}
</style>
