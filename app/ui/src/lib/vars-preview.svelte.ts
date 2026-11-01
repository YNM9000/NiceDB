import { SvelteMap } from 'svelte/reactivity';
import { api } from './api';
import type { VarPreview } from './bindings';
import { hasVars } from './vars';

type Scope = { id: string | null; folder: string | null };

const keyOf = (s: Scope, text: string) => `${s.id ?? ''}\u0000${s.folder ?? ''}\u0000${text}`;

export class VarPreviewer {
  #cache = new SvelteMap<string, VarPreview>();
  #queued = new Set<string>();
  #timer: ReturnType<typeof setTimeout> | undefined;
  #scope: () => Scope;

  constructor(scope: () => Scope) {
    this.#scope = scope;
  }

  of(text: string): VarPreview | undefined {
    if (!hasVars(text)) return undefined;
    const key = keyOf(this.#scope(), text);
    const hit = this.#cache.get(key);
    if (!hit && !this.#queued.has(text)) {
      this.#queued.add(text);
      clearTimeout(this.#timer);
      this.#timer = setTimeout(() => void this.#flush(), 120);
    }
    return hit;
  }

  async #flush() {
    const texts = [...this.#queued];
    this.#queued.clear();
    if (texts.length) await this.#fetch(texts);
  }

  async #fetch(texts: string[]) {
    const scope = this.#scope();
    try {
      const out = await api.varsPreview(scope.id, scope.folder, texts);
      texts.forEach((t, i) => {
        const p = out[i];
        if (p) this.#cache.set(keyOf(scope, t), p);
      });
    } catch {
    }
  }
}
