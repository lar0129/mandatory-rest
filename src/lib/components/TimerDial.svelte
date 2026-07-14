<script lang="ts">
  import { tweened } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';

  interface Props {
    elapsedSecs: number;
    totalSecs: number;
    color: string;
    identity: string;
    countdown?: boolean;
  }

  let { elapsedSecs, totalSecs, color, identity, countdown = false }: Props = $props();

  const CIRCUMFERENCE = 691.15;
  const dashOffset = tweened(CIRCUMFERENCE, { duration: 800, easing: cubicOut });
  let previousIdentity = $state('');

  $effect(() => {
    const progress = totalSecs > 0 ? elapsedSecs / totalSecs : 0;
    const target = countdown ? CIRCUMFERENCE * progress : CIRCUMFERENCE * (1 - progress);
    const startOffset = countdown ? 0 : CIRCUMFERENCE;

    if (identity !== previousIdentity) {
      dashOffset.set(startOffset, { duration: 0 });
      dashOffset.set(target, { duration: 360, easing: cubicOut });
      previousIdentity = identity;
    } else {
      dashOffset.set(target);
    }
  });
</script>

<svg class="dial" viewBox="0 0 230 230" aria-hidden="true">
  <path
    class="track"
    d="M115,5c60.8,0,110,49.2,110,110s-49.2,110-110,110S5,175.8,5,115S54.2,5,115,5"
    fill="none"
    stroke="var(--color-background-light)"
    stroke-width="2"
  />
  <path
    class="progress"
    d="M115,5c60.8,0,110,49.2,110,110s-49.2,110-110,110S5,175.8,5,115S54.2,5,115,5"
    fill="none"
    stroke={color}
    stroke-width="10"
    stroke-linecap="round"
    stroke-dasharray={CIRCUMFERENCE}
    stroke-dashoffset={$dashOffset}
  />
</svg>

<style>
  .dial {
    width: 220px;
    height: 220px;
    display: block;
  }

  .progress {
    transition: stroke var(--transition-slow);
  }
</style>
