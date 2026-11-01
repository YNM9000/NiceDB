import { m } from '../paraglide/messages';
import type { Report } from './bindings';

export function infoValue(report: Report, section: string, name: string): string | null {
  const s = report.sections.find((x) => x.id === section);
  return s?.rows.find((r) => r[0] === name)?.[1] ?? null;
}

export function keyspaceTotal(report: Report): number {
  const s = report.sections.find((x) => x.id === 'keyspace');
  return (s?.rows ?? []).reduce((sum, r) => sum + Number(/keys=(\d+)/.exec(r[1] ?? '')?.[1] ?? 0), 0);
}

const TITLES: Record<string, () => string> = {
  server: m.srv_sec_server,
  clients: m.srv_sec_clients,
  memory: m.srv_sec_memory,
  persistence: m.srv_sec_persistence,
  stats: m.srv_sec_stats,
  replication: m.srv_sec_replication,
  cpu: m.srv_sec_cpu,
  modules: m.srv_sec_modules,
  errorstats: m.srv_sec_errorstats,
  cluster: m.srv_sec_cluster,
  keyspace: m.srv_sec_keyspace,
  keysizes: m.srv_sec_keysizes,
  commandstats: m.srv_sec_commandstats,
  cluster_slots: m.srv_sec_cluster_slots,
  sentinel: m.srv_sec_sentinel,
};

export const sectionTitle = (id: string) => TITLES[id]?.() ?? id;
