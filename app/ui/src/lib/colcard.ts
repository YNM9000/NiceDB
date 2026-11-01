import { mount, unmount } from 'svelte';
import ColumnCard from '../components/ColumnCard.svelte';
import type { ColumnFacts, Describe } from './colinfo';

export function cardDom(facts: ColumnFacts, describe: Describe | null): { dom: HTMLElement; destroy: () => void } {
  const dom = document.createElement('div');
  dom.className = 'cm-column-card';
  const card = mount(ColumnCard, { target: dom, props: { facts, describe } });
  return { dom, destroy: () => void unmount(card) };
}
