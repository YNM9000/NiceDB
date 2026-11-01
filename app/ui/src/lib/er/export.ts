import { bounds, HEAD, ROW, type Point, type Rect } from './model';
import { areaBoxes, areaColor, compact, draw, edgeEnds, endShape, NO_MARKS, type Look, type Scene } from './render';

const PAD = 40;
const MAX_SIDE = 8192;

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const n = (v: number) => String(Math.round(v * 10) / 10);

function visible(s: Scene, hidden: ReadonlySet<string>) {
  return s.graph.tables.filter((t) => !hidden.has(t.id));
}

export function frame(s: Scene, hidden: ReadonlySet<string>): Rect {
  const tables = visible(s, hidden);
  return bounds([...tables, ...areaBoxes(s.graph, hidden)], PAD);
}

function path(pts: readonly Point[]) {
  return pts.map((p, i) => `${i ? 'L' : 'M'}${n(p.x)} ${n(p.y)}`).join('');
}

export function toSvg(s: Scene, look: Look, hidden: ReadonlySet<string> = new Set()): string {
  const box = frame(s, hidden);
  const c = look.c;
  const out: string[] = [
    `<svg xmlns="http://www.w3.org/2000/svg" width="${n(box.w)}" height="${n(box.h)}" viewBox="${n(box.x)} ${n(box.y)} ${n(box.w)} ${n(box.h)}">`,
    `<rect x="${n(box.x)}" y="${n(box.y)}" width="${n(box.w)}" height="${n(box.h)}" fill="${c.bg}"/>`,
  ];
  if (look.wash !== 'none')
    for (const a of areaBoxes(s.graph, hidden)) {
      const color = areaColor(look, a.area);
      const shape = `x="${n(a.x)}" y="${n(a.y)}" width="${n(a.w)}" height="${n(a.h)}"`;
      if (look.wash === 'outline') out.push(`<rect ${shape} fill="none" stroke="${c['ink-3']}" stroke-width="0.8" stroke-dasharray="12 4 2 4"/>`);
      else out.push(`<rect ${shape} rx="${look.wash === 'water' ? 18 : look.radius * 2}" fill="${color}" fill-opacity="${look.areaAlpha}" stroke="${color}" stroke-opacity="${Math.min(1, look.areaAlpha * 2)}"/>`);
      const label = s.areaLabels[a.area];
      if (!label) continue;
      const text = esc(look.caps ? label.toUpperCase() : label);
      const font = `font-family="${esc(look.fontArea)}" font-size="${look.areaSize}" font-style="${look.areaStyle.includes('italic') ? 'italic' : 'normal'}" fill="${c['area-ink']}"`;
      out.push(look.wash === 'outline' ? `<text x="${n(a.x + 2)}" y="${n(a.y - 5)}" ${font}>${text}</text>` : `<text x="${n(a.x + a.w - 12)}" y="${n(a.y + a.h - 9)}" text-anchor="end" ${font}>${text}</text>`);
    }

  const edges = s.graph.edges.filter((e) => !hidden.has(e.from) && !hidden.has(e.to) && e.route.length > 1);
  if (look.lineCaseW > 0) out.push(`<g fill="none" stroke="${c['line-case']}" stroke-width="${look.lineW + look.lineCaseW * 2}" stroke-linejoin="round">${edges.map((e) => `<path d="${path(e.route)}"/>`).join('')}</g>`);
  const ends: string[] = [];
  for (const e of edges) {
    const [a, b, cc, d] = [e.route[0], e.route[1], e.route.at(-2), e.route.at(-1)];
    const [ce, pe] = edgeEnds(e);
    for (const shape of [a && b ? endShape(a, b, ce) : null, cc && d ? endShape(d, cc, pe) : null]) {
      if (!shape) continue;
      for (const [x1, y1, x2, y2] of shape.lines) ends.push(`<line x1="${n(x1)}" y1="${n(y1)}" x2="${n(x2)}" y2="${n(y2)}"/>`);
      if (shape.ring) ends.push(`<circle cx="${n(shape.ring.x)}" cy="${n(shape.ring.y)}" r="${shape.ring.r}" fill="${c.bg}"/>`);
    }
  }
  out.push(`<g fill="none" stroke="${c.line}" stroke-width="${look.lineW}" stroke-linejoin="round">${edges.map((e) => `<path d="${path(e.route)}"/>`).join('')}</g>`);
  out.push(`<g stroke="${look.lineCaseW > 0 ? c['line-case'] : c.line}" stroke-width="${look.lineCaseW > 0 ? 1.6 : Math.max(1, look.lineW)}">${ends.join('')}</g>`);

  const title = `font-family="${esc(look.fontTitle)}" font-weight="${look.titleWeight}" font-size="13"`;
  const mono = `font-family="${esc(look.fontMono)}"`;
  for (const t of visible(s, hidden)) {
    const fk = s.fkColumns.get(t.id);
    const r = look.radius;
    out.push(`<g>`);
    out.push(`<rect x="${t.x}" y="${t.y}" width="${t.w}" height="${t.h}" rx="${r}" fill="${c.card}"/>`);
    out.push(`<path d="M${t.x} ${t.y + HEAD}V${t.y + r}q0 -${r} ${r} -${r}H${t.x + t.w - r}q${r} 0 ${r} ${r}V${t.y + HEAD}Z" fill="${c.head}"/>`);
    out.push(`<line x1="${t.x}" y1="${t.y + HEAD}" x2="${t.x + t.w}" y2="${t.y + HEAD}" stroke="${c.rule}" stroke-width="${look.frameW}"/>`);
    out.push(`<rect x="${t.x}" y="${t.y}" width="${t.w}" height="${t.h}" rx="${r}" fill="none" stroke="${c.frame}" stroke-width="${look.frameW}"/>`);
    out.push(`<text x="${t.x + 10}" y="${t.y + HEAD / 2 + 4.5}" ${title} fill="${c['head-ink']}">${esc(look.caps ? t.name.toUpperCase() : t.name)}</text>`);
    if (t.relation.estimated_rows !== null)
      out.push(`<text x="${t.x + t.w - 9}" y="${t.y + HEAD / 2 + 4}" ${mono} font-size="10.5" text-anchor="end" fill="${c.head === c.card ? c['ink-3'] : c['head-ink']}">${esc(compact(t.relation.estimated_rows))}</text>`);
    t.relation.columns.forEach((col, i) => {
      const y = t.y + HEAD + i * ROW + ROW / 2 + 4.5;
      if (col.primary_key || fk?.has(col.name))
        out.push(`<text x="${t.x + 7}" y="${y - 1}" ${mono} font-size="8.5" font-weight="600" fill="${col.primary_key ? c.pk : c.fk}">${col.primary_key ? 'PK' : 'FK'}</text>`);
      out.push(`<text x="${t.x + 26}" y="${y}" ${mono} font-size="12"${col.primary_key ? ' font-weight="500" text-decoration="underline"' : ''} fill="${col.primary_key ? c.ink : c['ink-2']}">${esc(col.name)}</text>`);
      out.push(`<text x="${t.x + t.w - 9}" y="${y}" ${mono} font-size="11" text-anchor="end" fill="${c['ink-3']}">${esc(col.type_name)}</text>`);
    });
    out.push(`</g>`);
  }
  out.push('</svg>');
  return out.join('\n');
}

export async function toPng(s: Scene, look: Look, hidden: ReadonlySet<string> = new Set()): Promise<Uint8Array> {
  const box = frame(s, hidden);
  const scale = Math.min(2, MAX_SIDE / Math.max(box.w, box.h));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(box.w * scale));
  canvas.height = Math.max(1, Math.round(box.h * scale));
  const ctx = canvas.getContext('2d', { alpha: false });
  if (!ctx) throw new Error('canvas');
  draw(ctx, s, look, { k: scale, x: -box.x * scale, y: -box.y * scale }, canvas.width, canvas.height, { ...NO_MARKS, hidden }, { lod: 1 });
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error('png');
  return new Uint8Array(await blob.arrayBuffer());
}
