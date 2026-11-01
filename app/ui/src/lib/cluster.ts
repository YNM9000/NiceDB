import type { Report, ServerInfo } from './bindings';
import { num, tag } from './format';
import { sizeText } from './value';
import { m } from '../paraglide/messages';

export interface Tile {
  label: string;
  big: string;
  facts: string[];
}

const SECTION: Record<string, () => string> = {
  health: m.cluster_sec_health,
  cluster: m.cluster_sec_cluster,
  region: m.cluster_sec_region,
  nodes: m.cluster_sec_nodes,
  ring: m.cluster_sec_ring,
  keyspaces: m.cluster_sec_keyspaces,
  indices: m.cluster_sec_indices,
  shards: m.cluster_sec_shards,
  tables: m.cluster_sec_tables,
};

export const sectionLabel = (id: string): string => (SECTION[id] ?? (() => id))();

function info(report: Report): Map<string, string> {
  const s = report.sections.find((x) => x.group === 'info');
  return new Map((s?.rows ?? []).map((r) => [r[0] ?? '', r[1] ?? '']));
}

const count = (report: Report, id: string) => report.sections.find((s) => s.id === id)?.rows.length ?? 0;
const n = (v: string | undefined) => (v === undefined || v === '' ? '—' : Number.isFinite(Number(v)) ? num(Number(v)) : v);

export function clusterTiles(report: Report, server: ServerInfo): Tile[] {
  const v = info(report);
  const version = { label: m.srv_version(), big: server.version || '—', facts: [server.product] };
  if (v.has('status')) {
    return [
      { label: m.cluster_health(), big: v.get('status') ?? '—', facts: [v.get('cluster_name') ?? ''].filter(Boolean) },
      version,
      { label: m.cluster_nodes(), big: n(v.get('number_of_nodes')), facts: [m.cluster_data_nodes({ count: n(v.get('number_of_data_nodes')) })] },
      { label: m.cluster_shards(), big: n(v.get('active_shards')), facts: [m.cluster_primaries({ count: n(v.get('active_primary_shards')) })] },
      { label: m.cluster_unassigned(), big: n(v.get('unassigned_shards')), facts: [m.cluster_relocating({ count: n(v.get('relocating_shards')) }), m.cluster_initializing({ count: n(v.get('initializing_shards')) })] },
    ];
  }
  if (v.has('region')) {
    const size = Number(v.get('size_bytes') ?? 0);
    return [
      { label: m.ddb_region(), big: v.get('region') ?? '—', facts: [v.get('endpoint') ?? ''].filter(Boolean) },
      version,
      { label: m.cluster_tables(), big: n(v.get('tables')), facts: [] },
      { label: m.cluster_items(), big: n(v.get('items')), facts: [m.cluster_items_note()] },
      { label: m.cluster_size(), big: sizeText(size, tag()), facts: [] },
    ];
  }
  return [
    { label: m.cluster_name(), big: v.get('cluster_name') || '—', facts: [v.get('partitioner')?.split('.').pop() ?? ''].filter(Boolean) },
    version,
    { label: m.cluster_nodes(), big: n(v.get('nodes')), facts: [m.cluster_ranges({ count: num(count(report, 'ring')) })] },
    { label: m.cql_consistency(), big: v.get('consistency') ?? '—', facts: [m.cluster_consistency_note()] },
    { label: m.cluster_keyspaces(), big: num(count(report, 'keyspaces')), facts: [] },
  ];
}
