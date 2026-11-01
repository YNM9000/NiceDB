import { SvelteSet } from 'svelte/reactivity';
import { m } from '../paraglide/messages';
import { api, type SessionInfo } from './api';
import type { KeyInfo_Serialize as KeyInfo, KeyValue_Serialize as KeyValue, KeyWrite_Deserialize as KeyWrite, Outcome } from './bindings';
import { connections } from './connections.svelte';
import { askConfirm, writesOpen } from './guard.svelte';
import { describeWrite, foldersOf, mergeKeys, SEPARATOR } from './keys';
import { globEscape, type KeyLookup } from './redis';
import { play } from './sound';

export const PAGE = 1000;
export const VALUE_PAGE = 200;
export const LOAD_ALL_CAP = 200_000;

const errorText = (e: unknown) => (e instanceof Error ? e.message : typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : String(e));

export class KeyBrowserState {
  pattern = $state('');
  kind = $state('');
  keys = $state.raw<KeyInfo[]>([]);
  cursor = $state('0');
  total = $state<number | null>(null);
  scanning = $state(false);
  started = $state(false);
  error = $state<string | null>(null);
  open = new SvelteSet<string>();
  selected = $state<string | null>(null);
  value = $state.raw<KeyValue | null>(null);
  valueBusy = $state(false);
  valueError = $state<string | null>(null);
  private run = 0;

  get done() {
    return this.started && this.cursor === '0';
  }

  async reload(sid: string) {
    this.run++;
    this.keys = [];
    this.cursor = '0';
    this.started = false;
    await this.more(sid);
  }

  async more(sid: string, count = PAGE) {
    if (this.scanning) return;
    this.scanning = true;
    this.error = null;
    const run = this.run;
    try {
      const page = await api.keysScan(sid, { pattern: this.pattern.trim() || '*', cursor: this.started ? this.cursor : '0', count, kind: this.kind || null });
      if (run !== this.run) return;
      this.keys = mergeKeys(this.keys, page.keys);
      this.cursor = page.cursor;
      this.total = page.total;
      this.started = true;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.scanning = false;
    }
  }

  async all(sid: string) {
    const run = ++this.run;
    while (run === this.run && !this.done && !this.error && this.keys.length < LOAD_ALL_CAP) {
      await this.more(sid, 5000);
    }
  }

  stop() {
    this.run++;
  }

  toggle(path: string) {
    if (this.open.has(path)) this.open.delete(path);
    else this.open.add(path);
  }

  reveal(key: string) {
    for (const f of foldersOf(key)) this.open.add(f);
  }

  async select(sid: string, key: string | null) {
    this.selected = key;
    this.value = null;
    this.valueError = null;
    if (key !== null) await this.readValue(sid);
  }

  async readValue(sid: string, more = false) {
    const key = this.selected;
    if (key === null) return;
    this.valueBusy = true;
    try {
      const cursor = more ? (this.value?.next ?? '') : '';
      const next = await api.keyRead(sid, { key, cursor, count: VALUE_PAGE });
      if (this.selected !== key) return;
      const before = this.value;
      if (more && before && before.data.shape === 'entries' && next.data.shape === 'entries') {
        this.value = { ...next, data: { shape: 'entries', entries: [...before.data.entries, ...next.data.entries] } };
      } else this.value = next;
      this.valueError = null;
      this.keys = this.keys.map((k) => (k.key === key ? { ...k, ttl_ms: next.ttl_ms, kind: next.kind } : k));
    } catch (e) {
      this.valueError = errorText(e);
    } finally {
      this.valueBusy = false;
    }
  }

  applied(w: KeyWrite) {
    if (w.op === 'delete') {
      const gone = new Set(w.keys);
      this.keys = this.keys.filter((k) => !gone.has(k.key));
      if (this.selected && gone.has(this.selected)) {
        this.selected = null;
        this.value = null;
      }
    } else if (w.op === 'rename') {
      this.keys = this.keys.map((k) => (k.key === w.key ? { ...k, key: w.to } : k));
      if (this.selected === w.key) this.selected = w.to;
      this.reveal(w.to);
    } else if (w.op === 'create') {
      this.keys = mergeKeys(this.keys, [{ key: w.key, kind: w.kind, ttl_ms: w.ttl_ms ?? null, bytes: null }]);
      this.reveal(w.key);
      this.selected = w.key;
    } else if (w.op === 'expire') {
      this.keys = this.keys.map((k) => (k.key === w.key ? { ...k, ttl_ms: w.ttl_ms ?? null } : k));
    }
  }
}

const states = new Map<string, KeyBrowserState>();

export function keyBrowser(tabId: string): KeyBrowserState {
  let s = states.get(tabId);
  if (!s) {
    s = new KeyBrowserState();
    states.set(tabId, s);
  }
  return s;
}

export const folderPattern = (prefix: string) => `${globEscape(prefix.endsWith(SEPARATOR) ? prefix : `${prefix}${SEPARATOR}`)}*`;

const lookups = new Map<string, KeyLookup>();

export function keyLookup(sid: string): KeyLookup {
  let f = lookups.get(sid);
  if (!f) {
    const cache = new Map<string, { at: number; names: Promise<string[]> }>();
    f = (prefix: string) => {
      const hit = cache.get(prefix);
      if (hit && Date.now() - hit.at < 5000) return hit.names;
      const names = api.keysScan(sid, { pattern: `${globEscape(prefix)}*`, cursor: '0', count: 200, kind: null }).then((p) => p.keys.map((k) => k.key).sort());
      cache.set(prefix, { at: Date.now(), names });
      if (cache.size > 100) cache.delete(cache.keys().next().value ?? '');
      return names;
    };
    lookups.set(sid, f);
  }
  return f;
}

export type Done = { ok: true; outcome: Outcome } | { ok: false; message: string };

export async function writeKey(session: SessionInfo, write: KeyWrite): Promise<Done | null> {
  let confirm: string | null = null;
  if (session.environment === 'production') {
    if (!writesOpen(session.session_id)) return { ok: false, message: m.data_ro_locked() };
    const typed = await askConfirm({
      title: m.kv_confirm_title({ key: write.op === 'delete' ? (write.keys.length === 1 ? (write.keys[0] ?? '') : m.kv_keys_count({ count: write.keys.length })) : write.key }),
      connection: connections.list.find((c) => c.id === session.connection_id)?.name ?? '',
      production: true,
      lines: describeWrite(write),
      risks: [],
      note: '',
      action: m.confirm_apply_btn(),
    });
    if (typed === false) return null;
    confirm = typed;
  }
  try {
    const outcome = await api.keyWrite(session.session_id, write, confirm);
    play('applied');
    return { ok: true, outcome };
  } catch (e) {
    return { ok: false, message: errorText(e) };
  }
}
