import type { PdfBlock, PdfReport_Deserialize as PdfReport, PdfStyle, PdfTone, SecCategory, SecFinding_Serialize as Finding, SecSeverity, SecSkip, SecSubject_Serialize as Subject, SecurityReport_Serialize as Report } from './bindings';
import { m } from '../paraglide/messages';

export type { Finding, Report };

const SEVERITY_ORDER: Record<SecSeverity, number> = { high: 0, medium: 1, low: 2 };
const CATEGORY_ORDER: SecCategory[] = ['accounts', 'network', 'version', 'data', 'hygiene'];

export const SEVERITY: Record<SecSeverity, () => string> = { high: m.sec_sev_high, medium: m.sec_sev_medium, low: m.sec_sev_low };
export const SEVERITY_GROUP: Record<SecSeverity, () => string> = { high: m.sec_group_high, medium: m.sec_group_medium, low: m.sec_group_low };
export const CATEGORY: Record<SecCategory, () => string> = {
  accounts: m.sec_cat_accounts,
  network: m.sec_cat_network,
  version: m.sec_cat_version,
  data: m.sec_cat_data,
  hygiene: m.sec_cat_hygiene,
};
export const SKIP: Record<SecSkip, () => string> = { engine: m.sec_skip_engine, privilege: m.sec_skip_privilege, not_detectable: m.sec_skip_not_detectable };

const TITLE: Record<string, () => string> = {
  superusers: m.sec_c_superusers_title,
  default_accounts: m.sec_c_default_accounts_title,
  anonymous_accounts: m.sec_c_anonymous_accounts_title,
  weak_passwords: m.sec_c_weak_passwords_title,
  public_grants: m.sec_c_public_grants_title,
  ddl_rights: m.sec_c_ddl_rights_title,
  hba_trust: m.sec_c_hba_trust_title,
  hba_md5: m.sec_c_hba_md5_title,
  hba_open: m.sec_c_hba_open_title,
  ssl: m.sec_c_ssl_title,
  listen_all: m.sec_c_listen_all_title,
  server_version: m.sec_c_server_version_title,
  sensitive_columns: m.sec_c_sensitive_columns_title,
  logging: m.sec_c_logging_title,
  backup_recent: m.sec_c_backup_recent_title,
  risky_features: m.sec_c_risky_features_title,
};

const WHAT: Record<string, () => string> = {
  superusers: m.sec_c_superusers_what,
  default_accounts: m.sec_c_default_accounts_what,
  anonymous_accounts: m.sec_c_anonymous_accounts_what,
  weak_passwords: m.sec_c_weak_passwords_what,
  public_grants: m.sec_c_public_grants_what,
  ddl_rights: m.sec_c_ddl_rights_what,
  hba_trust: m.sec_c_hba_trust_what,
  hba_md5: m.sec_c_hba_md5_what,
  hba_open: m.sec_c_hba_open_what,
  ssl: m.sec_c_ssl_what,
  listen_all: m.sec_c_listen_all_what,
  server_version: m.sec_c_server_version_what,
  sensitive_columns: m.sec_c_sensitive_columns_what,
  logging: m.sec_c_logging_what,
  backup_recent: m.sec_c_backup_recent_what,
  risky_features: m.sec_c_risky_features_what,
};

export const titleOf = (id: string) => TITLE[id]?.() ?? id;
export const whatOf = (id: string) => WHAT[id]?.() ?? '';

export interface Counts {
  high: number;
  medium: number;
  low: number;
  passed: number;
  skipped: number;
}

export function counts(report: Report): Counts {
  const c: Counts = { high: 0, medium: 0, low: 0, passed: 0, skipped: 0 };
  for (const f of report.checks) {
    if (f.verdict === 'passed') c.passed++;
    else if (f.verdict === 'skipped') c.skipped++;
    else if (f.severity) c[f.severity]++;
  }
  return c;
}

export function found(report: Report): Finding[] {
  return report.checks
    .filter((f) => f.verdict === 'found' && f.severity)
    .sort((a, b) => SEVERITY_ORDER[a.severity ?? 'low'] - SEVERITY_ORDER[b.severity ?? 'low'] || CATEGORY_ORDER.indexOf(a.category) - CATEGORY_ORDER.indexOf(b.category));
}

export const passed = (report: Report) => report.checks.filter((f) => f.verdict === 'passed');
export const skipped = (report: Report) => report.checks.filter((f) => f.verdict === 'skipped');

export const nothingToCheck = (report: Report) => report.checks.every((f) => f.verdict === 'skipped' || f.id === 'sensitive_columns' || f.id === 'server_version');

const KINDS: Record<string, () => string> = { cf: m.sec_d_cf, iban: m.sec_d_iban, email: m.sec_d_email, phone: m.sec_d_phone };

export function detailText(id: string, s: Subject): string {
  const d = s.detail ?? '';
  if (id === 'weak_passwords') return d === 'empty' ? m.sec_d_empty() : d === 'policy' ? m.sec_d_policy() : d === 'plaintext' ? m.sec_d_plaintext() : m.sec_d_hash();
  if (id === 'sensitive_columns') {
    const [kind = '', values] = d.split('+');
    return `${KINDS[kind]?.() ?? kind} · ${values ? m.sec_d_values() : m.sec_d_name()}`;
  }
  if (id === 'server_version') {
    if (d === 'eol') return m.sec_d_eol_old();
    if (d.startsWith('eol ')) return m.sec_d_eol({ date: d.slice(4) });
    const [fixed = '', , summary = ''] = d.split(' | ');
    return `${m.sec_d_fixed({ version: fixed })} · ${summary}`;
  }
  if (id === 'backup_recent') return /^\d+$/.test(d) ? m.sec_d_days({ days: d }) : d === 'never' ? m.sec_d_never() : d;
  if (id === 'risky_features') return d === 'extension' ? m.sec_d_extension() : d === 'language' ? m.sec_d_language() : d;
  return d;
}

export function subjectName(id: string, s: Subject): string {
  return id.startsWith('hba_') && /^\d+$/.test(s.name) ? m.sec_d_line({ line: s.name }) : s.name;
}

export function subjectLine(id: string, s: Subject): string {
  const text = detailText(id, s);
  return text ? `${subjectName(id, s)} — ${text}` : subjectName(id, s);
}

export function dateStamp(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export interface Meta {
  connection: string;
  date: string;
}

const PASSWORD_TOKEN = '{{password}}';

export const fixText = (fix: string): string => fix.replaceAll(PASSWORD_TOKEN, m.sec_password_hint());

const fence = (sql: string) => `\`\`\`sql\n${sql}\n\`\`\``;

export function markdownReport(report: Report, meta: Meta): string {
  const c = counts(report);
  const out: string[] = [`# ${m.sec_title()} — ${meta.connection}`, '', m.sec_server({ product: report.product, version: report.version, database: report.database, date: meta.date }), '', `> ${m.sec_report_head()}`, '', `**${m.sec_summary({ high: c.high, medium: c.medium, low: c.low, passed: c.passed, skipped: c.skipped })}**`, ''];
  const list = found(report);
  for (const sev of ['high', 'medium', 'low'] as const) {
    const group = list.filter((f) => f.severity === sev);
    if (!group.length) continue;
    out.push(`## ${SEVERITY_GROUP[sev]()} (${group.length})`, '');
    for (const f of group) {
      out.push(`### ${titleOf(f.id)}`, '', `*${CATEGORY[f.category]()} · ${SEVERITY[sev]()}*`, '', whatOf(f.id), '');
      if (f.subjects.length) out.push(`**${m.sec_where()}**`, '', ...f.subjects.map((s) => `- ${subjectLine(f.id, s)}`), '');
      if (f.fix) out.push(`**${m.sec_fix()}**`, '', fence(fixText(f.fix)), '');
    }
  }
  if (!list.length) out.push(m.sec_none(), '');
  const ok = passed(report);
  if (ok.length) out.push(`## ${m.sec_passed()} (${ok.length})`, '', ...ok.map((f) => `- ${titleOf(f.id)}`), '');
  const no = skipped(report);
  if (no.length) out.push(`## ${m.sec_skipped()} (${no.length})`, '', ...no.map((f) => `- ${titleOf(f.id)}: ${f.reason ? SKIP[f.reason]() : ''}`), '');
  out.push('---', '', m.sec_snapshot({ date: report.versions_snapshot }), '', `${m.sec_sources()}:`, '', ...[...new Set(report.versions_sources)].map((u) => `- ${u}`), '');
  return out.join('\n');
}

const TONE: Record<SecSeverity, PdfTone> = { high: 'high', medium: 'medium', low: 'low' };

export function pdfReport(report: Report, meta: Meta, style: PdfStyle, language: string): PdfReport {
  const c = counts(report);
  const tally = (value: number, label: string, tone: PdfTone) => ({ value: String(value), label, tone: value ? tone : ('muted' as const) });
  const blocks: PdfBlock[] = [];
  const list = found(report);
  for (const sev of ['high', 'medium', 'low'] as const) {
    const group = list.filter((f) => f.severity === sev);
    if (!group.length) continue;
    blocks.push({ kind: 'heading', text: SEVERITY_GROUP[sev](), count: String(group.length), tone: TONE[sev] });
    for (const f of group)
      blocks.push({
        kind: 'finding',
        severity: SEVERITY[sev](),
        tone: TONE[sev],
        category: CATEGORY[f.category](),
        title: titleOf(f.id),
        what: whatOf(f.id),
        subjects_label: m.sec_where(),
        subjects: f.subjects.map((s) => subjectLine(f.id, s)),
        fix_label: m.sec_fix(),
        fix: f.fix === null ? null : fixText(f.fix),
      });
  }
  if (!list.length) blocks.push({ kind: 'paragraph', text: m.sec_none(), tone: 'ok' });
  const ok = passed(report);
  if (ok.length) blocks.push({ kind: 'heading', text: m.sec_passed(), count: String(ok.length), tone: 'plain' }, { kind: 'list', items: ok.map((f) => titleOf(f.id)), tone: 'ok' });
  const no = skipped(report);
  if (no.length) blocks.push({ kind: 'heading', text: m.sec_skipped(), count: String(no.length), tone: 'plain' }, { kind: 'list', items: no.map((f) => `${titleOf(f.id)}: ${f.reason ? SKIP[f.reason]() : ''}`), tone: 'muted' });
  blocks.push({ kind: 'paragraph', text: `${m.sec_snapshot({ date: report.versions_snapshot })} ${m.sec_sources()}: ${[...new Set(report.versions_sources)].join(' · ')}`, tone: 'muted' });
  return {
    style,
    language,
    title: `${m.sec_title()} — ${meta.connection}`,
    subtitle: m.sec_server({ product: report.product, version: report.version, database: report.database, date: meta.date }),
    lead: m.sec_report_head(),
    tally: [tally(c.high, SEVERITY.high(), 'high'), tally(c.medium, SEVERITY.medium(), 'medium'), tally(c.low, SEVERITY.low(), 'low'), tally(c.passed, m.sec_passed(), 'ok'), tally(c.skipped, m.sec_skipped(), 'muted')],
    blocks,
    footer: m.sec_pdf_footer({ connection: meta.connection }),
    pages: m.sec_pdf_pages({ n: '{n}', total: '{total}' }),
  };
}
