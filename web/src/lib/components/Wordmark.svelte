<script lang="ts">
	let { size = 'md' }: { size?: 'md' | 'lg' } = $props();
</script>

<span class="wordmark {size}" translate="no">Qobuz<span class="tail">DLX</span></span>

<style>
	.wordmark {
		font-family: var(--font-content);
		font-weight: 600;
		letter-spacing: -0.01em;
		color: var(--text);
		line-height: 1;
	}

	.md {
		font-size: 1.25rem;
	}

	.lg {
		font-size: 1.75rem;
	}

	.tail {
		font-weight: 400;
		font-style: italic;
		color: var(--text-muted);
		margin-left: 1px;
	}
</style>
