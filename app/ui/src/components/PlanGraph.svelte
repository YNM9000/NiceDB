<script lang="ts">
  import { BOX, clip, glyph, label, layout, ms, rows, short, subject, type Plan } from '../lib/plan';
  import { m } from '../paraglide/messages';

  let { plan, selected, onselect }: { plan: Plan; selected: number | null; onselect: (id: number | null) => void } = $props();
  let width = $state(0);

  const l = $derived(layout(plan));
  const scale = $derived(width > 0 ? Math.max(0.84, Math.min(1, (width - 2) / Math.max(1, l.width))) : 1);
  const hotBox = $derived(l.boxes.find((b) => b.hot) ?? null);
  const root = $derived(plan.nodes[0]);
  const W = BOX.w;
  const H = BOX.h;

  const opRoom = (op: string) => {
    const room = plan.analyzed ? 15 : 22;
    return /[a-z]/.test(op) ? room : Math.round(room * 0.78);
  };
  const offBy = (est: number, act: number | null) => Math.max(est, act ?? 0, 1) / Math.max(1, Math.min(est, act ?? 0));

  function fit(op: SVGTextElement, _text: string) {
    const measure = () => {
      op.removeAttribute('textLength');
      op.removeAttribute('lengthAdjust');
      const time = op.parentElement?.querySelector<SVGTextElement>('.pg-t');
      if (typeof op.getComputedTextLength !== 'function') return;
      const room = (time ? time.getBBox().x : W - 9) - op.getBBox().x - 6;
      if (room > 0 && op.getComputedTextLength() > room) {
        op.setAttribute('textLength', String(room));
        op.setAttribute('lengthAdjust', 'spacingAndGlyphs');
      }
    };
    measure();
    void document.fonts?.ready.then(measure);
    return { update: measure };
  }

  function key(e: KeyboardEvent, id: number) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      onselect(selected === id ? null : id);
    }
  }
</script>

<figure class="pg" bind:clientWidth={width}>
  <svg class="pg-svg" width={l.width * scale} height={l.height * scale} viewBox="0 0 {l.width} {l.height}" role="group" aria-label={m.plan_view_drawing()}>
    <defs>
      <marker id="pg-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
        <path d="M0 0L10 5L0 10z" class="pg-arrowhead" />
      </marker>
    </defs>
    {#each l.edges as e, i (i)}<path class="pg-casing" d={e.d} stroke-width={e.width + 4} />{/each}
    {#each l.edges as e, i (i)}<path class="pg-edge" class:hot={e.hot} d={e.d} stroke-width={e.width} marker-end={e.to === null ? 'url(#pg-arrow)' : undefined} />{/each}

    {#if l.cloud && hotBox}
      <path class="pg-cloud" d={l.cloud} />
      <g class="pg-delta" transform="translate({hotBox.x - 18} {hotBox.y - 24})">
        <path d="M0 14L8 0L16 14z" />
        <text x="8" y="12" text-anchor="middle">1</text>
      </g>
      <ellipse class="pg-ring" cx={hotBox.x + W / 2} cy={hotBox.y + H / 2} rx={W / 2 + 14} ry={H / 2 + 9} />
    {/if}

    {#each l.boxes as b (b.id)}
      {@const n = plan.nodes[b.id]}
      {#if n}
        {@const g = glyph(n.kind)}
        <g
          class="pg-node"
          class:hot={b.hot}
          class:worst={b.worst}
          class:sel={selected === b.id}
          transform="translate({b.x} {b.y})"
          role="button"
          tabindex="0"
          aria-label={label(n)}
          aria-pressed={selected === b.id}
          onclick={() => onselect(selected === b.id ? null : b.id)}
          onkeydown={(e) => key(e, b.id)}
        >
          <rect class="pg-box" width={W} height={H} />
          <circle class="pg-lamp" cx="13" cy="15" r="4" />
          <g class="pg-glyph g-{g}" transform="translate(13 15)">
            {#if g === 'scan'}<rect x="-4.5" y="-4.5" width="9" height="9" />
            {:else if g === 'index'}<rect x="-4.5" y="-4.5" width="9" height="9" /><path d="M-4.5 1.5H4.5" />
            {:else if g === 'join'}<circle r="5" /><circle r="1.6" class="dot" />
            {:else if g === 'aggregate'}<path d="M0 -5.5L5.5 0L0 5.5L-5.5 0z" />
            {:else if g === 'sort'}<path d="M0 -5L5.5 4.5H-5.5z" />
            {:else}<circle r="3" class="dot" />{/if}
          </g>
          <text class="pg-op" x="24" y="19" use:fit={`${n.op} ${n.self_ms}`}>{clip(n.op, opRoom(n.op))}</text>
          {#if plan.analyzed}<text class="pg-t" x={W - 9} y="19" text-anchor="end">{ms(n.self_ms)}</text>{/if}
          <text class="pg-sub" x="10" y="37">{clip(subject(n), /[a-z]/.test(subject(n)) ? 25 : 20)}</text>
          <text class="pg-rows" x="10" y="54">{plan.analyzed ? rows(n.rows, short) : m.plan_est_rows({ count: short(n.est_rows) })}</text>
          {#if plan.analyzed && n.est_rows !== null && (b.worst || offBy(n.est_rows, n.rows) >= 2)}<text class="pg-est" x={W - 9} y="54" text-anchor="end">{m.plan_est_rows({ count: short(n.est_rows) })}</text>{/if}
          <rect class="pg-track" x="10" y={H - 6} width={W - 20} height="2" />
          <rect class="pg-share" x="10" y={H - 6} width={Math.max(b.share > 0 ? 1.5 : 0, (W - 20) * b.share)} height="2" />
          <g class="pg-balloon" transform="translate({W} 0)">
            <circle r="9" />
            <text y="3.5" text-anchor="middle">{b.n}</text>
          </g>
        </g>
      {/if}
    {/each}

    <g class="pg-exit" transform="translate({l.exit.x + 6} {l.exit.y - 24})">
      <rect width={BOX.exit - 6} height="48" />
      <text class="pg-exit-k" x={(BOX.exit - 6) / 2} y="15" text-anchor="middle">{m.plan_result()}</text>
      <text class="pg-exit-v" x={(BOX.exit - 6) / 2} y="31" text-anchor="middle">{rows(root?.rows ?? root?.est_rows)}</text>
      {#if plan.total_ms !== null}<text class="pg-exit-s" x={(BOX.exit - 6) / 2} y="43" text-anchor="middle">{m.plan_in_total({ time: ms(plan.total_ms) })}</text>{/if}
    </g>
  </svg>
  <figcaption class="pg-legend">
    <span><i class="pg-sw-width"></i>{m.plan_legend_width()}</span>
    {#if plan.analyzed}<span><i class="pg-sw-slow"></i>{m.plan_legend_slow()}</span>{/if}
    <span>{m.plan_legend_flow()}</span>
  </figcaption>
</figure>
