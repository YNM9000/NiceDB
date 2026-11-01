import type { SearchDone, SearchPlanned } from './api';
import type { Match, SearchSpec, TableHits } from './bindings';

export interface KeptSearch {
  value: string;
  mode: Match;
  caseSensitive: boolean;
  kinds: SearchSpec['kinds'];
  kindsTouched: boolean;
  chosen: string[] | null;
  filter: string;
  views: boolean;
  rowLimit: number;
  budget: number;
  plan: SearchPlanned | null;
  results: TableHits[];
  failed: { name: string; message: string }[];
  done: SearchDone | null;
  stopped: boolean;
  closed: Set<string>;
  searched: SearchSpec | null;
}

const kept = new WeakMap<object, KeptSearch>();

export const keptSearch = (tab: object): KeptSearch | undefined => kept.get(tab);
export const keepSearch = (tab: object, state: KeptSearch) => void kept.set(tab, state);
