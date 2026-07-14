<script lang="ts">
  import { fade } from 'svelte/transition';
  import {
    pauseRestReminder,
    previewRestReminder,
    resetRestReminder,
    resumeRestReminder,
    timerRestartRound,
    timerSkip,
    timerToggle,
  } from '$lib/ipc';
  import { restReminderState } from '$lib/stores/restReminder';
  import { timerState } from '$lib/stores/timer';

  interface Props {
    activeView: 'pomodoro' | 'rest';
  }

  let { activeView }: Props = $props();
  let state = $derived($timerState);
  let rest = $derived($restReminderState);
  let restDisabled = $derived(!rest.enabled || rest.active || rest.pause_reason === 'pomodoro');

  function toggleRest() {
    if (restDisabled) return;
    if (rest.pause_reason === 'manual') {
      void resumeRestReminder();
    } else {
      void pauseRestReminder();
    }
  }
</script>

<div class="mini-controls">
  {#if activeView === 'pomodoro'}
    <button class="btn-side" onclick={timerRestartRound} aria-label="Restart round">
      <svg width="10" height="10" viewBox="0 0 16 16">
        <polygon points="15,1 6,8 15,15" fill="currentColor" />
        <rect x="1" y="1" width="3" height="14" rx="1" fill="currentColor" />
      </svg>
    </button>

    <button class="play-pause" onclick={timerToggle} aria-label={state.is_running ? 'Pause' : 'Play'}>
      {#key state.is_running}
        <span class="icon" in:fade={{ duration: 100 }}>
          {#if state.is_running}
            <svg width="12" height="12" viewBox="0 0 24 24">
              <rect x="4" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
              <rect x="15" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
            </svg>
          {:else}
            <svg width="12" height="12" viewBox="0 0 24 24">
              <polygon points="5,3 21,12 5,21" fill="currentColor" />
            </svg>
          {/if}
        </span>
      {/key}
    </button>

    <button class="btn-side" onclick={timerSkip} aria-label="Skip round">
      <svg width="10" height="10" viewBox="0 0 16 16">
        <polygon points="1,1 10,8 1,15" fill="currentColor" />
        <rect x="12" y="1" width="3" height="14" rx="1" fill="currentColor" />
      </svg>
    </button>
  {:else}
    <button
      class="btn-side"
      onclick={resetRestReminder}
      disabled={!rest.enabled || rest.active}
      aria-label="Reset forced rest"
    >
      <svg width="12" height="12" viewBox="0 0 24 24" fill="none">
        <path
          d="M4.8 8.2A8 8 0 1 1 4 15M4 4v5h5"
          stroke="currentColor"
          stroke-width="2.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    </button>

    <button
      class="play-pause rest-action"
      onclick={toggleRest}
      disabled={restDisabled}
      aria-label={rest.pause_reason === 'manual' ? 'Resume forced rest' : 'Pause forced rest'}
    >
      {#if rest.pause_reason === 'manual'}
        <svg width="12" height="12" viewBox="0 0 24 24">
          <polygon points="5,3 21,12 5,21" fill="currentColor" />
        </svg>
      {:else}
        <svg width="12" height="12" viewBox="0 0 24 24">
          <rect x="4" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
          <rect x="15" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
        </svg>
      {/if}
    </button>

    <button
      class="btn-side"
      onclick={previewRestReminder}
      disabled={!rest.enabled || rest.active}
      aria-label="Preview forced rest"
    >
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none">
        <path
          d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z"
          stroke="currentColor"
          stroke-width="2"
        />
        <circle cx="12" cy="12" r="2.7" fill="currentColor" />
      </svg>
    </button>
  {/if}
</div>

<style>
  .mini-controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .btn-side {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: none;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    color: var(--color-foreground-darker, var(--color-foreground));
    transition:
      color var(--transition-default),
      background var(--transition-default),
      opacity var(--transition-default);
  }

  .btn-side:hover:not(:disabled) {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .play-pause {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    background: none;
    border: 1.5px solid var(--color-foreground-darker, var(--color-foreground));
    border-radius: 50%;
    cursor: pointer;
    color: var(--color-foreground);
    overflow: hidden;
    transition:
      color var(--transition-default),
      border-color var(--transition-default),
      background var(--transition-default),
      opacity var(--transition-default);
  }

  .play-pause:hover:not(:disabled) {
    color: var(--color-accent);
    border-color: var(--color-accent);
    background: var(--color-hover);
  }

  .rest-action {
    color: var(--color-short-round);
    border-color: var(--color-short-round);
  }

  button:disabled {
    cursor: default;
    opacity: 0.28;
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }
</style>
