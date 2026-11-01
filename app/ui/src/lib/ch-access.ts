import type { ProfileSetting_Serialize as ProfileSetting, Quota, RowPolicy_Serialize as RowPolicy, SettingsProfile_Serialize as SettingsProfile } from './bindings';
import { quoteName } from './sqlname';

const q = (name: string) => quoteName('clickhouse', name);

function target(o: { all: boolean; applies_to: string[]; except: string[] }): string {
  if (o.all) return o.except.length ? ` TO ALL EXCEPT ${o.except.map(q).join(', ')}` : ' TO ALL';
  return o.applies_to.length ? ` TO ${o.applies_to.map(q).join(', ')}` : '';
}

const to = (who: string | null) => (who ? ` TO ${q(who)}` : '');

const str = (v: string) => `'${v.replaceAll('\\', '\\\\').replaceAll("'", "\\'")}'`;

const value = (v: string) => (/^-?\d+(\.\d+)?$/.test(v) ? v : str(v));

const capKeyword = (what: string) => what.replace(/^max_/, '');

function seconds(n: number): string {
  if (n % 86400 === 0) return `${n / 86400} day`;
  if (n % 3600 === 0) return `${n / 3600} hour`;
  if (n % 60 === 0) return `${n / 60} minute`;
  return `${n} second`;
}

export function quotaCreate(name: string, who: string | null): string {
  return `CREATE QUOTA ${q(name)} FOR INTERVAL 1 hour MAX queries = 1000, read_rows = 1000000000${to(who)}`;
}

export function quotaAlter(qt: Quota): string {
  const windows = qt.limits
    .filter((l): l is typeof l & { seconds: number } => l.caps.length > 0 && l.seconds !== null)
    .map((l) => `FOR INTERVAL ${seconds(l.seconds)} MAX ${l.caps.map((c) => `${capKeyword(c.what)} = ${c.max}`).join(', ')}`);
  return `ALTER QUOTA ${q(qt.name)}${windows.length ? ` ${windows.join(', ')}` : ''}${target(qt)}`;
}

export const quotaDrop = (qt: Quota) => `DROP QUOTA ${q(qt.name)}`;

function setting(s: ProfileSetting): string {
  const parts = [q(s.name)];
  if (s.value !== null) parts.push(`= ${value(s.value)}`);
  if (s.min !== null) parts.push(`MIN ${value(s.min)}`);
  if (s.max !== null) parts.push(`MAX ${value(s.max)}`);
  if (s.writability) parts.push(s.writability);
  return parts.join(' ');
}

export function profileCreate(name: string, who: string | null): string {
  return `CREATE SETTINGS PROFILE ${q(name)} SETTINGS max_memory_usage = 10000000000, max_execution_time = 300${to(who)}`;
}

export function profileAlter(p: SettingsProfile): string {
  const items = [...p.settings.map(setting), ...p.inherits.map((i) => `INHERIT ${str(i)}`)];
  return `ALTER SETTINGS PROFILE ${q(p.name)}${items.length ? ` SETTINGS ${items.join(', ')}` : ''}${target(p)}`;
}

export const profileDrop = (p: SettingsProfile) => `DROP SETTINGS PROFILE ${q(p.name)}`;

const on = (r: { database: string; table: string }) => `${q(r.database)}.${q(r.table)}`;

export function policyCreate(name: string, who: string | null, table: { database: string; table: string }): string {
  return `CREATE ROW POLICY ${q(name)} ON ${on(table)} FOR SELECT USING 1 AS permissive${to(who)}`;
}

export function policyAlter(r: RowPolicy): string {
  return `ALTER ROW POLICY ${q(r.name)} ON ${on(r)} FOR SELECT USING ${r.filter ?? '1'} AS ${r.restrictive ? 'restrictive' : 'permissive'}${target(r)}`;
}

export const policyDrop = (r: RowPolicy) => `DROP ROW POLICY ${q(r.name)} ON ${on(r)}`;

export const appliesTo = (o: { all: boolean; applies_to: string[]; except: string[] }, name: string) => (o.all ? !o.except.includes(name) : o.applies_to.includes(name));
