import { Briefcase, Building2, FlaskConical, Server, Shield, Star } from '@lucide/svelte';
import type { Component } from 'svelte';
import { m } from '../paraglide/messages';
import type { FolderMark, FolderTint } from './bindings';
import TintDot from '../components/TintDot.svelte';

type Icon = Component<{ size?: number; strokeWidth?: number }>;

export const TINTS: { id: FolderTint; label: () => string }[] = [
  { id: 'ochre', label: m.org_tint_ochre },
  { id: 'red', label: m.org_tint_red },
  { id: 'green', label: m.org_tint_green },
  { id: 'blue', label: m.org_tint_blue },
  { id: 'violet', label: m.org_tint_violet },
  { id: 'slate', label: m.org_tint_slate },
];

export const MARKS: { id: FolderMark; icon: Icon; label: () => string }[] = [
  { id: 'star', icon: Star, label: m.org_mark_star },
  { id: 'briefcase', icon: Briefcase, label: m.org_mark_briefcase },
  { id: 'building', icon: Building2, label: m.org_mark_building },
  { id: 'flask', icon: FlaskConical, label: m.org_mark_flask },
  { id: 'server', icon: Server, label: m.org_mark_server },
  { id: 'shield', icon: Shield, label: m.org_mark_shield },
];

export const markIcon = (mark: FolderMark | null): Icon | null => MARKS.find((x) => x.id === mark)?.icon ?? null;

export function tintIcon(tint: FolderTint | null): Icon {
  return ((internals: Parameters<typeof TintDot>[0], props: { size?: number }) => TintDot(internals, { size: props.size, tint })) as unknown as Icon;
}
