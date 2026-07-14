<script lang="ts">
  interface Props {
    remainingSecs: number;
    disabled?: boolean;
  }

  let { remainingSecs, disabled = false }: Props = $props();

  let remaining = $derived(Math.max(0, remainingSecs));
  let minutes = $derived(Math.floor(remaining / 60));
  let seconds = $derived(remaining % 60);
  let display = $derived(
    disabled ? '--:--' : `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
  );
</script>

<div class="display">
  <span class="time" class:disabled>{display}</span>
</div>

<style>
  .display {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .time {
    font-family: 'Mona Sans Mono', monospace;
    font-size: 2.8rem;
    font-weight: 300;
    font-stretch: 85%;
    letter-spacing: -0.02em;
    color: var(--color-foreground);
    transition: opacity var(--transition-default);
  }

  .time.disabled {
    opacity: 0.42;
  }
</style>
