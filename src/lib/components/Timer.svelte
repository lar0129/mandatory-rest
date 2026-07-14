<script lang="ts">
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import {
    getRestReminderState,
    getTimerState,
    notificationShow,
    onRestPauseWarning,
    onRestReminderState,
    onRoundChange,
    onTimerPaused,
    onTimerReset,
    onTimerResumed,
    onTimerTick,
    pauseRestReminder,
    previewRestReminder,
    resetRestReminder,
    resumeRestReminder,
    timerRestartRound,
    timerSkip,
    timerToggle,
  } from '$lib/ipc';
  import { restReminderState } from '$lib/stores/restReminder';
  import { settings } from '$lib/stores/settings';
  import { timerState } from '$lib/stores/timer';
  import * as m from '$paraglide/messages.js';
  import MiniControls from './MiniControls.svelte';
  import TimerDial from './TimerDial.svelte';
  import TimerDisplay from './TimerDisplay.svelte';
  import TimerFooter from './TimerFooter.svelte';
  import Tooltip from './Tooltip.svelte';

  interface Props {
    isCompact?: boolean;
    uiScale?: number;
  }

  type TimerView = 'pomodoro' | 'rest';

  let { isCompact = false, uiScale = 1 }: Props = $props();
  let activeView = $state<TimerView>('pomodoro');
  let timer = $derived($timerState);
  let rest = $derived($restReminderState);
  let pomodoroRemaining = $derived(Math.max(0, timer.total_secs - timer.elapsed_secs));
  let activeRemaining = $derived(
    activeView === 'pomodoro' ? pomodoroRemaining : rest.timer_remaining_secs
  );
  let activeTotal = $derived(
    activeView === 'pomodoro' ? timer.total_secs : rest.timer_total_secs
  );
  let activeElapsed = $derived(Math.max(0, activeTotal - activeRemaining));
  let activeColor = $derived(
    activeView === 'pomodoro' ? roundColor(timer.round_type) : 'var(--color-short-round)'
  );
  let activeIdentity = $derived(
    activeView === 'pomodoro' ? `pomodoro-${timer.round_type}` : 'forced-rest'
  );
  let restControlsDisabled = $derived(
    !rest.enabled || rest.active || rest.pause_reason === 'pomodoro'
  );

  function roundColor(roundType: string): string {
    if (roundType === 'work') return 'var(--color-focus-round)';
    if (roundType === 'short-break') return 'var(--color-short-round)';
    return 'var(--color-long-round)';
  }

  function roundLabel(roundType: string): string {
    if (roundType === 'work') return m.round_label_work();
    if (roundType === 'short-break') return m.round_label_short_break();
    return m.round_label_long_break();
  }

  function formatTime(totalSeconds: number): string {
    const safe = Math.max(0, totalSeconds);
    const minutes = Math.floor(safe / 60);
    const seconds = safe % 60;
    return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  function restStatus(): string {
    if (!rest.enabled) return m.rest_status_disabled();
    if (rest.active) return m.rest_status_overlay();
    if (rest.pause_reason === 'pomodoro') return m.rest_status_pomodoro_paused();
    if (rest.pause_reason === 'manual') return m.rest_status_manual_paused();
    return m.rest_status_counting();
  }

  async function toggleRestTimer() {
    if (restControlsDisabled) return;
    if (rest.pause_reason === 'manual') {
      await resumeRestReminder();
    } else {
      await pauseRestReminder();
    }
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];

    (async () => {
      const [initialTimer, initialRest] = await Promise.all([
        getTimerState(),
        getRestReminderState(),
      ]);
      timerState.set(initialTimer);
      restReminderState.set(initialRest);

      cleanups.push(
        await onTimerTick(({ elapsed_secs, total_secs }) => {
          timerState.update((current) => ({
            ...current,
            elapsed_secs,
            total_secs,
            is_running: true,
            is_paused: false,
          }));
        }),
        await onTimerPaused(({ elapsed_secs }) => {
          timerState.update((current) => ({
            ...current,
            elapsed_secs,
            is_running: false,
            is_paused: true,
          }));
        }),
        await onTimerResumed(({ elapsed_secs }) => {
          timerState.update((current) => ({
            ...current,
            elapsed_secs,
            is_running: true,
            is_paused: false,
          }));
        }),
        await onRoundChange((snapshot) => {
          timerState.set(snapshot);
          if ($settings.notifications_enabled) {
            let title: string;
            let body: string;
            if (snapshot.round_type === 'work') {
              const afterBreak =
                snapshot.previous_round_type === 'short-break' ||
                snapshot.previous_round_type === 'long-break';
              title = afterBreak ? m.notification_work_title() : m.notification_work_start_title();
              body = afterBreak ? m.notification_work_body() : m.notification_work_start_body();
            } else if (snapshot.round_type === 'short-break') {
              title = m.notification_short_break_title();
              body = m.notification_short_break_body();
            } else {
              title = m.notification_long_break_title();
              body = m.notification_long_break_body();
            }
            notificationShow(title, body).catch(() => {});
          }
        }),
        await onTimerReset((snapshot) => {
          timerState.set(snapshot);
        }),
        await onRestReminderState((snapshot) => {
          restReminderState.set(snapshot);
        }),
        await onRestPauseWarning(({ paused_secs }) => {
          const minutes = Math.max(1, Math.floor(paused_secs / 60));
          notificationShow(
            m.rest_pause_warning_title(),
            m.rest_pause_warning_body({ minutes })
          ).catch(() => {});
        })
      );
    })();

    return () => {
      for (const unlisten of cleanups) unlisten();
    };
  });
</script>

<div class="timer-outer" class:compact={isCompact}>
  <div class="timer" style="zoom: {uiScale}">
    <div class="view-switch" role="tablist" aria-label="Timer view">
      <button
        class:active={activeView === 'pomodoro'}
        role="tab"
        aria-selected={activeView === 'pomodoro'}
        onclick={() => (activeView = 'pomodoro')}
      >
        <span class="view-name">{m.rest_tab_pomodoro()}</span>
        <span class="view-time">{formatTime(pomodoroRemaining)}</span>
      </button>
      <button
        class:active={activeView === 'rest'}
        role="tab"
        aria-selected={activeView === 'rest'}
        onclick={() => (activeView = 'rest')}
      >
        <span class="view-name">{m.rest_tab_background()}</span>
        <span class="view-time">{rest.enabled ? formatTime(rest.timer_remaining_secs) : '--:--'}</span>
      </button>
    </div>

    {#key activeView}
      <div class="dial-stack" in:fade={{ duration: 160 }}>
        <TimerDial
          elapsedSecs={activeElapsed}
          totalSecs={activeTotal}
          color={activeColor}
          identity={activeIdentity}
          countdown={$settings.dial_countdown}
        />
        <TimerDisplay
          remainingSecs={activeRemaining}
          disabled={activeView === 'rest' && !rest.enabled}
        />
      </div>
    {/key}

    {#if !isCompact}
      <div
        class="round-label"
        class:paused={activeView === 'rest' && rest.is_paused}
        style="color: {activeColor}"
      >
        {activeView === 'pomodoro' ? roundLabel(timer.round_type) : restStatus()}
      </div>

      {#if activeView === 'pomodoro'}
        <div class="controls-wrapper" in:fade={{ duration: 140 }}>
          <Tooltip text={m.tooltip_restart_round()}>
            <button class="btn-side" onclick={timerRestartRound} aria-label="Restart round">
              <svg width="18" height="18" viewBox="0 0 16 16">
                <polygon points="15,1 6,8 15,15" fill="currentColor" />
                <rect x="1" y="1" width="3" height="14" rx="1" fill="currentColor" />
              </svg>
            </button>
          </Tooltip>

          <button
            class="play-pause"
            onclick={timerToggle}
            aria-label={timer.is_running ? 'Pause' : 'Play'}
          >
            {#key timer.is_running}
              <span class="icon" in:fade={{ duration: 120 }}>
                {#if timer.is_running}
                  <svg width="24" height="24" viewBox="0 0 24 24">
                    <rect x="5" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                    <rect x="14" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                  </svg>
                {:else}
                  <svg width="18" height="18" viewBox="0 0 24 24" style="overflow: visible;">
                    <polygon points="4,0 28,12 4,24" fill="currentColor" />
                  </svg>
                {/if}
              </span>
            {/key}
          </button>

          <Tooltip text={m.tooltip_skip()}>
            <button class="btn-side" onclick={timerSkip} aria-label="Skip round">
              <svg width="18" height="18" viewBox="0 0 16 16">
                <polygon points="1,1 10,8 1,15" fill="currentColor" />
                <rect x="12" y="1" width="3" height="14" rx="1" fill="currentColor" />
              </svg>
            </button>
          </Tooltip>

          <TimerFooter snap={timer} />
        </div>
      {:else}
        <div class="rest-control-area" in:fade={{ duration: 140 }}>
          <div class="rest-controls">
            <Tooltip text={m.rest_tooltip_reset()}>
              <button
                class="btn-side"
                onclick={resetRestReminder}
                disabled={!rest.enabled || rest.active}
                aria-label={m.rest_tooltip_reset()}
              >
                <svg width="19" height="19" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M4.8 8.2A8 8 0 1 1 4 15"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                  />
                  <path d="M4 4v5h5" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                </svg>
              </button>
            </Tooltip>

            <Tooltip
              text={rest.pause_reason === 'manual'
                ? m.rest_tooltip_resume()
                : m.rest_tooltip_pause()}
            >
              <button
                class="play-pause rest-action"
                onclick={toggleRestTimer}
                disabled={restControlsDisabled}
                aria-label={rest.pause_reason === 'manual' ? 'Resume forced rest' : 'Pause forced rest'}
              >
                {#if rest.pause_reason === 'manual'}
                  <svg width="18" height="18" viewBox="0 0 24 24" style="overflow: visible;">
                    <polygon points="4,0 28,12 4,24" fill="currentColor" />
                  </svg>
                {:else}
                  <svg width="24" height="24" viewBox="0 0 24 24">
                    <rect x="5" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                    <rect x="14" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                  </svg>
                {/if}
              </button>
            </Tooltip>

            <Tooltip text={m.rest_preview()}>
              <button
                class="btn-side"
                onclick={previewRestReminder}
                disabled={!rest.enabled || rest.active}
                aria-label={m.rest_preview()}
              >
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
                  <path
                    d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z"
                    stroke="currentColor"
                    stroke-width="1.8"
                  />
                  <circle cx="12" cy="12" r="2.7" fill="currentColor" />
                </svg>
              </button>
            </Tooltip>
          </div>

          <div class="rest-footer">
            {#if rest.pause_reason === 'manual' && rest.next_pause_warning_secs > 0}
              {m.rest_next_warning({ time: formatTime(rest.next_pause_warning_secs) })}
            {:else}
              {restStatus()}
            {/if}
          </div>
        </div>
      {/if}
    {/if}
  </div>

  {#if isCompact}
    <MiniControls {activeView} />
  {/if}
</div>

<style>
  .timer-outer {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .timer {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .view-switch {
    display: grid;
    grid-template-columns: 1fr 1fr;
    width: 250px;
    padding: 4px;
    border: 1px solid color-mix(in oklch, var(--color-foreground) 9%, transparent);
    border-radius: 999px;
    background: color-mix(in oklch, var(--color-background-light) 52%, transparent);
    box-shadow: inset 0 1px 2px color-mix(in oklch, #000 14%, transparent);
  }

  .view-switch button {
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 6px 10px;
    border: 0;
    border-radius: 999px;
    background: transparent;
    color: var(--color-foreground-darker, var(--color-foreground));
    cursor: pointer;
    font: inherit;
    transition:
      color var(--transition-snappy),
      background var(--transition-snappy),
      box-shadow var(--transition-snappy),
      transform var(--transition-snappy);
  }

  .view-switch button:hover {
    color: var(--color-foreground);
  }

  .view-switch button:active {
    transform: scale(0.98);
  }

  .view-switch button.active {
    color: var(--color-foreground);
    background: color-mix(in oklch, var(--color-background-light) 88%, var(--color-foreground) 12%);
    box-shadow:
      0 2px 8px color-mix(in oklch, #000 18%, transparent),
      inset 0 1px color-mix(in oklch, var(--color-foreground) 10%, transparent);
  }

  .view-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.69rem;
    font-weight: 600;
    letter-spacing: 0.025em;
  }

  .view-time {
    font-family: 'Mona Sans Mono', monospace;
    font-size: 0.66rem;
    font-variant-numeric: tabular-nums;
    opacity: 0.74;
  }

  .dial-stack {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .controls-wrapper {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 4px 12px;
  }

  .controls-wrapper > :global(*) {
    aspect-ratio: 1;
  }

  .btn-side {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--color-foreground-darker, var(--color-foreground));
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 6px;
    transition:
      color var(--transition-default),
      background var(--transition-default),
      opacity var(--transition-default);
  }

  .btn-side:hover:not(:disabled) {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .btn-side:disabled,
  .play-pause:disabled {
    cursor: default;
    opacity: 0.28;
  }

  .play-pause {
    background: none;
    border: 2px solid var(--color-foreground-darker, var(--color-foreground));
    cursor: pointer;
    color: var(--color-foreground);
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border-radius: 50%;
    transition:
      color var(--transition-default),
      border-color var(--transition-default),
      background var(--transition-default),
      opacity var(--transition-default);
    overflow: hidden;
  }

  .play-pause:hover:not(:disabled) {
    color: var(--color-accent);
    border-color: var(--color-accent);
    background: var(--color-hover);
  }

  .rest-action {
    color: var(--color-short-round);
    border-color: color-mix(in oklch, var(--color-short-round) 78%, var(--color-foreground));
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .round-label {
    min-height: 15px;
    margin-top: -4px;
    font-size: 0.72rem;
    font-weight: 650;
    letter-spacing: 0.075em;
    text-transform: uppercase;
    transition:
      color var(--transition-slow),
      opacity var(--transition-default);
  }

  .round-label.paused {
    opacity: 0.68;
  }

  .rest-control-area {
    min-height: 80px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .rest-controls {
    display: grid;
    grid-template-columns: 32px 48px 32px;
    align-items: center;
    gap: 12px;
  }

  .rest-footer {
    min-height: 18px;
    color: var(--color-foreground-darker, var(--color-foreground));
    font-size: 0.67rem;
    letter-spacing: 0.02em;
    opacity: 0.72;
  }

  .compact .view-switch {
    width: 190px;
  }

  .compact .view-time {
    display: none;
  }
</style>
