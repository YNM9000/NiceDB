const WRAPPERS: Record<string, string> = {
  $oid: 'objectId',
  $date: 'date',
  $numberDecimal: 'decimal',
  $numberLong: 'long',
  $numberInt: 'int',
  $numberDouble: 'double',
  $binary: 'binData',
  $timestamp: 'timestamp',
  $regularExpression: 'regex',
  $symbol: 'symbol',
  $code: 'javascript',
  $minKey: 'minKey',
  $maxKey: 'maxKey',
  $uuid: 'uuid',
};

export function wrapperType(v: unknown): string | null {
  if (v === null || typeof v !== 'object' || Array.isArray(v)) return null;
  const keys = Object.keys(v);
  const first = keys[0];
  if (!first || !(first in WRAPPERS)) return null;
  if (keys.length > 1 && !(first === '$code' && keys.length === 2 && 'scope' in v)) return null;
  return WRAPPERS[first] ?? null;
}

export function bsonType(v: unknown): string {
  const w = wrapperType(v);
  if (w) return w;
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  if (typeof v === 'object') return 'document';
  if (typeof v === 'number') return Number.isInteger(v) ? 'int' : 'double';
  if (typeof v === 'boolean') return 'bool';
  return typeof v === 'string' ? 'string' : 'unknown';
}

export function wrapperText(v: unknown): string {
  const o = v as Record<string, unknown>;
  const inner = Object.values(o)[0];
  if ('$date' in o) {
    if (typeof inner === 'string') return inner;
    const ms = Number((inner as Record<string, unknown> | null)?.$numberLong ?? inner);
    return Number.isFinite(ms) ? new Date(ms).toISOString() : JSON.stringify(inner);
  }
  if ('$binary' in o) {
    const b = inner as { base64?: string; subType?: string } | null;
    return `${b?.base64 ?? ''}${b?.subType ? ` (${b.subType})` : ''}`;
  }
  if ('$regularExpression' in o) {
    const r = inner as { pattern?: string; options?: string } | null;
    return `/${r?.pattern ?? ''}/${r?.options ?? ''}`;
  }
  if ('$timestamp' in o) {
    const t = inner as { t?: number; i?: number } | null;
    return `${t?.t ?? 0}:${t?.i ?? 0}`;
  }
  return typeof inner === 'string' || typeof inner === 'number' ? String(inner) : JSON.stringify(inner);
}

export function cellText(v: unknown): string | null {
  if (v === undefined || v === null) return null;
  if (wrapperType(v)) return wrapperText(v);
  if (typeof v === 'object') return JSON.stringify(v);
  return String(v);
}

export const idOf = (doc: unknown): string | null => {
  if (doc === null || typeof doc !== 'object' || !('_id' in doc)) return null;
  return JSON.stringify((doc as { _id: unknown })._id);
};

export function summaryParts(doc: unknown, fields = 4, naming: readonly string[] = ['_id']): [string, string][] {
  if (doc === null || typeof doc !== 'object') return [['', String(doc)]];
  return Object.entries(doc)
    .filter(([k]) => !naming.includes(k))
    .slice(0, fields)
    .map(([k, v]) => [k, typeof v === 'object' && v !== null && !wrapperType(v) ? (Array.isArray(v) ? `[${v.length}]` : '{…}') : (cellText(v) ?? 'null')]);
}

export const summary = (doc: unknown, fields = 3): string =>
  summaryParts(doc, fields)
    .map(([k, v]) => `${k}: ${v}`)
    .join(' · ');
