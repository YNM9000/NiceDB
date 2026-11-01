export interface Prepared {
  readonly lower: string;
  readonly mask: number;
}

export interface Query {
  readonly terms: readonly Prepared[];
  readonly mask: number;
}

const fold = (s: string) => s.toLowerCase();

function maskOf(lower: string): number {
  let mask = 0;
  for (let i = 0; i < lower.length; i++) {
    const c = lower.charCodeAt(i);
    if (c >= 97 && c <= 122) mask |= 1 << (c - 97);
    else if (c >= 48 && c <= 57) mask |= 1 << 26;
    else if (c === 95) mask |= 1 << 27;
    else if (c > 127) mask |= 1 << 28;
  }
  return mask;
}

export function prepare(text: string): Prepared {
  const lower = fold(text);
  return { lower, mask: maskOf(lower) };
}

export function parseQuery(text: string): Query {
  const terms = fold(text).split(/\s+/).filter(Boolean).map(prepare);
  return { terms, mask: terms.reduce((m, t) => m | t.mask, 0) };
}

const isBoundary = (s: string, i: number) => {
  if (i === 0) return true;
  const prev = s.charCodeAt(i - 1);
  return !((prev >= 97 && prev <= 122) || (prev >= 48 && prev <= 57) || prev > 127);
};

export function scoreTerm(term: string, text: string): number {
  if (!term) return 1;
  if (term.length > text.length) return 0;
  if (text === term) return 1000;
  const at = text.indexOf(term);
  if (at === 0) return 900 - Math.min(100, text.length - term.length);
  if (at > 0) {
    let word = at;
    while (word >= 0 && !isBoundary(text, word)) word = text.indexOf(term, word + 1);
    if (word > 0) return 700 - Math.min(60, word) - Math.min(60, text.length - term.length);
    return 500 - Math.min(80, at) - Math.min(60, text.length - term.length);
  }
  return subsequence(term, text);
}

function subsequence(term: string, text: string): number {
  let score = 200;
  let ti = 0;
  let last = -2;
  let first = -1;
  for (let i = 0; i < text.length && ti < term.length; i++) {
    if (text[i] !== term[ti]) continue;
    if (first < 0) first = i;
    if (i === last + 1) score += 12;
    else if (isBoundary(text, i)) score += 8;
    else score -= Math.min(10, i - last);
    last = i;
    ti++;
  }
  if (ti < term.length) return 0;
  score -= Math.min(40, first) + Math.min(40, text.length - term.length) / 2;
  return Math.max(1, Math.min(350, score));
}

export interface Candidate {
  readonly name: Prepared;
  readonly context: Prepared;
}

function match(q: Query, c: Candidate): number {
  if ((q.mask & ~(c.name.mask | c.context.mask)) !== 0) return 0;
  const lone = q.terms.length === 1;
  let total = 0;
  let named = false;
  for (const t of q.terms) {
    const own = (t.mask & ~c.name.mask) === 0 ? scoreTerm(t.lower, c.name.lower) : 0;
    if (lone && own <= 0) return 0;
    const around = own >= 700 || (t.mask & ~c.context.mask) !== 0 ? 0 : scoreTerm(t.lower, c.context.lower) * 0.4;
    if (own <= 0 && around <= 0) return 0;
    if (own >= around) named = true;
    total += Math.max(own, around);
  }
  return named ? total : -1;
}

export const scoreCandidate = (q: Query, c: Candidate): number => Math.max(0, match(q, c));

export interface Ranked<T> {
  item: T;
  score: number;
}

const normal = (query: string) => fold(query).trimStart().replace(/\s+/g, ' ');

const narrowed = new WeakMap<readonly Candidate[], { query: string; terms: number; kept: readonly Candidate[] }>();

export function rank<T extends Candidate>(items: readonly T[], query: string, boost: (item: T) => number, limit: number): Ranked<T>[] {
  return drain(rankSteps(items, query, boost, limit));
}

export function drain<R>(steps: Generator<void, R>): R {
  for (;;) {
    const step = steps.next();
    if (step.done) return step.value;
  }
}

const STEP = 4096;

export function* rankSteps<T extends Candidate>(items: readonly T[], query: string, boost: (item: T) => number, limit: number): Generator<void, Ranked<T>[]> {
  const q = parseQuery(query);
  const typed = normal(query);
  const memo = narrowed.get(items);
  const from = memo && memo.query && typed.startsWith(memo.query) && (memo.terms > 1 || q.terms.length === 1) ? (memo.kept as readonly T[]) : items;
  const kept: T[] = [];
  const out: Ranked<T>[] = [];
  let floor = -Infinity;
  for (let i = 0; i < from.length; i++) {
    if (i % STEP === STEP - 1) yield;
    const item = from[i] as T;
    const s = q.terms.length ? match(q, item) : 1;
    if (s === 0) continue;
    kept.push(item);
    if (s < 0) continue;
    const score = s + boost(item);
    if (out.length >= limit && score <= floor) continue;
    out.push({ item, score });
    if (out.length >= limit * 4) {
      out.sort((a, b) => b.score - a.score);
      out.length = limit;
      floor = out[limit - 1]?.score ?? -Infinity;
    }
  }
  narrowed.set(items, { query: q.terms.length ? typed : '', terms: q.terms.length, kept });
  out.sort((a, b) => b.score - a.score);
  if (out.length > limit) out.length = limit;
  return out;
}

export function inSlices<R>(steps: Generator<void, R>, done: (result: R) => void, sliceMs = 8): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const slice = () => {
    const until = performance.now() + sliceMs;
    for (;;) {
      const step = steps.next();
      if (step.done) return done(step.value);
      if (performance.now() > until) {
        timer = setTimeout(slice, 0);
        return;
      }
    }
  };
  timer = setTimeout(slice, 0);
  return () => clearTimeout(timer);
}

export function highlight(text: string, query: string): [number, number] | null {
  const q = parseQuery(query);
  const lower = fold(text);
  for (const t of q.terms) {
    const at = lower.indexOf(t.lower);
    if (at >= 0) return [at, at + t.lower.length];
  }
  return null;
}
