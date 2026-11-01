<script lang="ts">
  import { Braces, Columns3, Database, Hash, KeyRound, Link2, Sigma, Table2 } from '@lucide/svelte';
  import { splashCue, splashPlan, SKIP_FADE_MS, STAGE_H, STAGE_W, type SplashMode } from '../lib/splash';
  import { BEAT, cellCenter, cellRight, choreography, COL_X, DIGIT_H, ENGINES, FIELD, MONOGRAM, scene } from '../lib/splash-scene';
  import { smoothBackdrop } from '../lib/backdrop-canvas';
  import { flyMark, pickTarget, SKIP_EASE, SKIP_TRAVEL_MS, TRAVEL_EASE, TRAVEL_MS, type Flight } from '../lib/mark-flight';
  import type { JingleCue } from '../lib/sound';
  import DriverMark from './DriverMark.svelte';
  import Wordmark from './Wordmark.svelte';

  let { mode, onend }: { mode: SplashMode; onend: () => void } = $props();

  let root: HTMLElement;
  let backdrop: HTMLCanvasElement;
  let timers: number[] = [];
  let leaving = false;
  let flight: Flight | null = null;
  let cue: JingleCue | null = null;
  let k = $state(fit());
  const reduced = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
  const plan = splashPlan(reduced);
  const { cells, links } = scene();
  const cg = choreography();
  const icons = [Table2, KeyRound, Hash, Link2, Columns3, Sigma, Braces, Database];
  const engineRow = ENGINES.map((e, i) => {
    const away = Math.abs(i - (ENGINES.length - 1) / 2);
    return { e, x: (i - (ENGINES.length - 1) / 2) * 44, d: BEAT.engines + Math.round(away * 20) };
  });

  function fit() {
    return Math.min(innerWidth / STAGE_W, innerHeight / STAGE_H);
  }

  function detach() {
    timers.forEach(clearTimeout);
    removeEventListener('keydown', skip, true);
    removeEventListener('pointerdown', skip, true);
  }

  function fade(fadeMs: number) {
    root.style.setProperty('--sp-fade', `${fadeMs}ms`);
    root.dataset.leaving = '';
    timers = [window.setTimeout(onend, fadeMs + 20)];
  }

  function travel(ms: number, ease: string) {
    const source = root.querySelector<SVGSVGElement>('.sp-word .wordmark');
    const target = pickTarget(document);
    if (!source || !target) return false;
    flight = flyMark(source, [...root.querySelectorAll<HTMLElement>('.sp-total')], target, ms, ease);
    root.style.setProperty('--sp-fade', `${Math.round(ms * 0.8)}ms`);
    root.dataset.flying = '';
    root.dataset.leaving = '';
    void flight.done.then(onend);
    return true;
  }

  function leave(skipped: boolean) {
    if (leaving) return;
    leaving = true;
    detach();
    if (skipped) cue?.land(0);
    if (reduced) return fade(skipped ? SKIP_FADE_MS : plan.fadeMs);
    if (!travel(skipped ? SKIP_TRAVEL_MS : TRAVEL_MS, skipped ? SKIP_EASE : TRAVEL_EASE)) fade(skipped ? SKIP_FADE_MS : plan.fadeMs);
  }

  function skip() {
    leave(true);
  }

  export function start() {
    root.dataset.run = '';
    cue = splashCue();
    addEventListener('keydown', skip, true);
    addEventListener('pointerdown', skip, true);
    if (mode === 'hold') {
      timers = [window.setTimeout(() => (root.dataset.done = ''), plan.holdMs)];
      return;
    }
    timers = [window.setTimeout(() => leave(false), plan.holdMs)];
  }

  $effect(() => smoothBackdrop(backdrop, root));

  $effect(() => () => {
    detach();
    flight?.cancel();
    cue?.cancel();
  });
</script>

<svelte:window onresize={() => (k = fit())} />

<div class="splash" bind:this={root} aria-hidden="true">
  <canvas class="sp-backdrop" bind:this={backdrop}></canvas>
  <div class="sp-stage" style:--k={k} style:--oy="{cg.originY}px" style:--digit-h="{DIGIT_H}px">
    <div class="sp-camera" style:--d={0} style:--t={BEAT.fold}>
      <div class="sp-brace" style:--d={BEAT.brace} style:--t={BEAT.fold + 60 - BEAT.brace}>
        {#each cg.cols as col, i (i)}
          <i class="sp-col" style:left="{col.x}px" style:top="{FIELD.y}px" style:height="{FIELD.rows * FIELD.rh}px" style:--d={col.out}
            ><i class="sp-col-line sp-enter-y" style:--from={col.from} style:--d={col.enter} style:--t={720}></i></i
          >
        {/each}
        {#each cg.rows as row, j (j)}
          <i class="sp-row" class:sp-edge={j === 0 || j === cg.rows.length - 1} style:left="{FIELD.x}px" style:top="{row.y}px" style:width="{FIELD.w}px" style:--dy="{row.dy}px" style:--d={row.out}
            ><i class="sp-row-line sp-enter-x" style:--from={row.from} style:--d={row.enter} style:--t={760}></i></i
          >
        {/each}

        <div class="sp-field" style:--d={BEAT.fold - 60}>
          <i class="sp-scan" style:left="{FIELD.x}px" style:top="{FIELD.y}px" style:width="{FIELD.w}px" style:height="{FIELD.rh}px" style:--to="{Math.floor(FIELD.rows / 2) * FIELD.rh}px" style:--d={BEAT.cells} style:--t={BEAT.brace - BEAT.cells}></i>
          {#each MONOGRAM as m, i (i)}
            <i class="sp-mono a-spring" class:sp-mono-diag={m.diag} style:left="{m.x}px" style:top="{m.y}px" style:width="{m.w}px" style:height="{m.h}px" style:--d={m.d} style:--t={420}></i>
          {/each}
          {#each COL_X.slice(0, -1) as x, i (i)}
            <span class="sp-index a-rise" style:left="{x + 8}px" style:top="{FIELD.y - 22}px" style:--d={BEAT.cols + 200 + i * 46}>{String(i + 1).padStart(2, '0')}</span>
          {/each}
          {#each Array.from({ length: FIELD.rows }, (_, j) => j) as j (j)}
            <span class="sp-index sp-index-row a-fade" style:left="{FIELD.x - 46}px" style:top="{FIELD.y + j * FIELD.rh + 13}px" style:--d={BEAT.rows + 260 + j * 26}>{String(j + 1).padStart(3, '0')}</span>
          {/each}

          <svg class="sp-links" width={STAGE_W} height={STAGE_H} viewBox="0 0 {STAGE_W} {STAGE_H}">
            {#each links as l, i (i)}
              <path class="a-draw" d={l.path} pathLength="1" style:--d={l.d} style:--t={l.t} />
              <circle class="a-pop" cx={l.ends[0]} cy={l.ends[1]} r="2.5" style:--d={l.d} style:--t={260} />
              <circle class="a-pop" cx={l.ends[2]} cy={l.ends[3]} r="2.5" style:--d={l.d + l.t - 80} style:--t={260} />
            {/each}
          </svg>

          {#each cells as cell, i (i)}
            {@const p = cellCenter(cell.c, cell.r)}
            {#if cell.kind === 'num'}
              <span class="sp-num a-fade" style:left="{cellRight(cell.c) - 14}px" style:top="{p.y}px" style:--d={cell.d} style:--t={200}
                >{#each cell.digits as dg, n (n)}{#if typeof dg === 'string'}<span class="sp-sep">{dg}</span>{:else}<span class="sp-digit"
                      ><span class="sp-strip" style:--n={dg.n} style:--d={cell.d} style:--t={dg.t}>{dg.strip}</span></span
                    >{/if}{/each}</span
              >
            {:else if cell.kind === 'glyph'}
              {@const Icon = icons[cell.glyph] ?? Table2}
              <span class="sp-glyph a-spring" style:left="{p.x}px" style:top="{p.y}px" style:--d={cell.d} style:--t={520}><Icon size={18} strokeWidth={1.5} /></span>
            {:else}
              <span class="sp-glyph sp-engine a-spring" style:left="{p.x}px" style:top="{p.y}px" style:--d={cell.d} style:--t={520}><DriverMark driver={cell.engine} size={18} framed={false} /></span>
            {/if}
          {/each}
        </div>
      </div>
    </div>

    <i class="sp-lead" style:left="{FIELD.x}px" style:top="{cg.originY}px" style:width="{FIELD.w}px" style:--d={BEAT.lead} style:--t={620}></i>


    {#each cg.totals as r, i (i)}
      <i class="sp-total" style:left="{r.x}px" style:top="{r.y}px" style:width="{r.w}px" style:height="{r.h}px" style:--dx="{r.dx}px" style:--sx={r.sx} style:--d={r.d} style:--t={BEAT.resolve + 540 - r.d}></i>
    {/each}

    <div class="sp-word" style:left="{cg.word.left}px" style:top="{cg.word.top}px" style:--d={BEAT.word} style:--t={760}>
      <Wordmark size={cg.word.size} text />
    </div>
    <div class="sp-engines" style:top="{cg.word.top + 168}px">
      {#each engineRow as m (m.e.id)}
        <span class="sp-glyph a-spring-up" style:left="{720 + m.x}px" style:--d={m.d} style:--t={400}><DriverMark driver={m.e} size={18} framed={false} /></span>
      {/each}
    </div>
  </div>
  <i class="sp-bunny a-rise" style:--d={BEAT.engines + 100} style:--t={440}></i>
</div>
