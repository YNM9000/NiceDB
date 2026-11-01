import type { ConnectionProfile, SecurityReport } from './api';
import { ApiError } from './api';
import type { PdfReport_Deserialize as PdfReport, SecFinding_Serialize as Finding, SecSkip } from './bindings';
import { mockServer } from './mock-drivers';

export const snapshot = { date: '2026-10-28' };

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Driver = ConnectionProfile['target']['driver'];

const PG: Driver[] = ['postgres'];
const SERVERS: Driver[] = ['postgres', 'cockroach', 'mysql', 'sqlserver', 'clickhouse'];
const NOT_CRDB: Driver[] = ['postgres', 'mysql', 'sqlserver', 'clickhouse'];
const EVERY: Driver[] = ['postgres', 'cockroach', 'mysql', 'sqlserver', 'sqlite', 'duckdb', 'libsql', 'clickhouse'];

const APPLIES: Record<string, Driver[]> = {
  superusers: SERVERS,
  default_accounts: SERVERS,
  anonymous_accounts: ['mysql'],
  weak_passwords: SERVERS,
  public_grants: ['postgres', 'cockroach', 'sqlserver'],
  ddl_rights: SERVERS,
  hba_trust: PG,
  hba_md5: PG,
  hba_open: PG,
  ssl: NOT_CRDB,
  listen_all: ['postgres', 'mysql'],
  server_version: ['postgres', 'mysql', 'sqlserver', 'sqlite'],
  sensitive_columns: EVERY,
  logging: NOT_CRDB,
  backup_recent: SERVERS,
  risky_features: NOT_CRDB,
};

const ORDER = Object.keys(APPLIES);

const CATEGORY: Record<string, Finding['category']> = {
  superusers: 'accounts',
  default_accounts: 'accounts',
  anonymous_accounts: 'accounts',
  weak_passwords: 'accounts',
  public_grants: 'accounts',
  ddl_rights: 'accounts',
  hba_trust: 'network',
  hba_md5: 'network',
  hba_open: 'network',
  ssl: 'network',
  listen_all: 'network',
  server_version: 'version',
  sensitive_columns: 'data',
  logging: 'hygiene',
  backup_recent: 'hygiene',
  risky_features: 'hygiene',
};

type Hit = Pick<Finding, 'severity' | 'subjects' | 'fix'>;
const s = (name: string, detail: string | null = null) => ({ name, detail });

const HITS: Record<string, Hit> = {
  weak_passwords: {
    severity: 'high',
    subjects: [s('report', 'empty'), s('backup', 'hash')],
    fix: "ALTER ROLE \"report\" PASSWORD '{{password}}';\nSET password_encryption = 'scram-sha-256';\nALTER ROLE \"backup\" PASSWORD '{{password}}';",
  },
  public_grants: {
    severity: 'high',
    subjects: [s('public.listini', 'SELECT, INSERT, UPDATE'), s('public.tmp_import_2023', 'SELECT')],
    fix: 'REVOKE SELECT, INSERT, UPDATE ON TABLE "public"."listini" FROM PUBLIC;\nREVOKE SELECT ON TABLE "public"."tmp_import_2023" FROM PUBLIC;',
  },
  hba_trust: {
    severity: 'high',
    subjects: [s('84', 'host all all 10.0.0.0/8 trust'), s('86', 'local all postgres trust')],
    fix: '-- pg_hba.conf line 84: use scram-sha-256\n-- was: host all all 10.0.0.0/8 trust\n-- pg_hba.conf line 86: use scram-sha-256\n-- was: local all postgres trust\nSELECT pg_reload_conf();',
  },
  ssl: { severity: 'high', subjects: [s('ssl', 'off')], fix: '-- needs a certificate and a key (ssl_cert_file, ssl_key_file)\nALTER SYSTEM SET ssl = on;\nSELECT pg_reload_conf();' },
  server_version: {
    severity: 'high',
    subjects: [
      s('CVE-2026-16239', '16.15 | high | Type confusion with CLOSE and DECLARE on cursors, code execution on the server'),
      s('CVE-2026-15741', '16.15 | high | SQL injection nella ricostruzione dell\'argomento di EXTRACT'),
      s('CVE-2026-18024', '16.15 | medium | ascii() legge oltre la fine del buffer'),
    ],
    fix: '-- upgrade to at least version 16.15 (same series)',
  },
  superusers: { severity: 'medium', subjects: [s('nicedb'), s('postgres')], fix: 'ALTER ROLE "nicedb" NOSUPERUSER;\nALTER ROLE "postgres" NOSUPERUSER;' },
  ddl_rights: {
    severity: 'medium',
    subjects: [s('erp_web', 'CREATE'), s('mrossi', 'CREATEDB')],
    fix: 'REVOKE CREATE ON SCHEMA "public" FROM "erp_web";\nALTER ROLE "mrossi" NOCREATEDB NOCREATEROLE;',
  },
  hba_open: {
    severity: 'medium',
    subjects: [s('92', 'host all all all md5')],
    fix: "-- pg_hba.conf line 92: narrow the address and use hostssl\n-- was: host all all all md5\nSELECT pg_reload_conf();",
  },
  listen_all: { severity: 'medium', subjects: [s('listen_addresses', '*')], fix: "ALTER SYSTEM SET listen_addresses = 'localhost';\n-- needs a restart; for one network address: 'localhost,10.0.0.5'" },
  sensitive_columns: {
    severity: 'medium',
    subjects: [s('public.clienti.codice_fiscale', 'cf+values'), s('public.clienti.iban', 'iban+values'), s('public.clienti.email', 'email+values'), s('public.clienti.telefono', 'phone')],
    fix: 'ALTER TABLE "public"."clienti" ENABLE ROW LEVEL SECURITY;\n-- then a policy for the roles that must see the rows: CREATE POLICY name ON "public"."clienti" FOR SELECT TO role USING (...);',
  },
  backup_recent: {
    severity: 'medium',
    subjects: [s('archive_mode', 'on')],
    fix: "-- WAL archiving is on but has not archived for more than two days: check archive_command and the space at the destination",
  },
  logging: {
    severity: 'low',
    subjects: [s('log_connections', 'off'), s('log_disconnections', 'off')],
    fix: 'ALTER SYSTEM SET log_connections = on;\nALTER SYSTEM SET log_disconnections = on;\nSELECT pg_reload_conf();',
  },
};

const CLICKHOUSE_HITS: Record<string, Hit> = {
  weak_passwords: {
    severity: 'medium',
    subjects: [s('etl', 'plaintext'), s('report', 'plaintext')],
    fix: "ALTER USER `etl` IDENTIFIED WITH sha256_password BY '{{password}}';\nALTER USER `report` IDENTIFIED WITH sha256_password BY '{{password}}';",
  },
  ssl: {
    severity: 'high',
    subjects: [s('https_port')],
    fix: '<!-- /etc/clickhouse-server/config.d/tls.xml -->\n<clickhouse>\n  <https_port>8443</https_port>\n  <tcp_port_secure>9440</tcp_port_secure>\n</clickhouse>',
  },
  risky_features: {
    severity: 'medium',
    subjects: [s('etl', 'FILE, URL, S3')],
    fix: 'REVOKE FILE, URL, S3 ON *.* FROM `etl`;',
  },
  backup_recent: {
    severity: 'medium',
    subjects: [s('gestionale', 'never')],
    fix: "BACKUP DATABASE `gestionale` TO File('gestionale.zip');",
  },
};

const PASSING = new Set(['default_accounts', 'hba_md5', 'risky_features']);
const UNREADABLE = new Set(['weak_passwords']);

export interface SeclabMockOptions {
  profile: (sessionId: string) => ConnectionProfile | undefined;
}

export function seclabMock({ profile }: SeclabMockOptions) {
  return {
    reportPdf: async (_name: string, report: PdfReport): Promise<boolean> => {
      await sleep(60);
      (globalThis as { ndLastPdf?: PdfReport }).ndLastPdf = report;
      return true;
    },
    securityCheck: async (sid: string): Promise<SecurityReport> => {
      await sleep(240);
      const target = profile(sid)?.target;
      if (!target) throw new ApiError({ code: 'not_found', detail: sid });
      const driver = target.driver;
      const server = mockServer(target);
      const unreadable = new URLSearchParams(location.search).has('noread');
      const checks: Finding[] = ORDER.map((id) => {
        const base = { id, category: CATEGORY[id] ?? 'accounts', subjects: [], fix: null, note: null, severity: null, reason: null as SecSkip | null };
        if (!(APPLIES[id] ?? []).includes(driver)) return { ...base, verdict: 'skipped', reason: 'engine' };
        if (id === 'backup_recent' && driver === 'mysql') return { ...base, verdict: 'skipped', reason: 'not_detectable' };
        if (unreadable && UNREADABLE.has(id)) return { ...base, verdict: 'skipped', reason: 'privilege' };
        if (driver === 'clickhouse') {
          const ch = CLICKHOUSE_HITS[id];
          return ch ? { ...base, verdict: 'found', ...ch, note: null, reason: null } : { ...base, verdict: 'passed' };
        }
        const hit = HITS[id];
        if (PASSING.has(id) || !hit || !SERVERS.includes(driver)) return { ...base, verdict: 'passed' };
        return { ...base, verdict: 'found', ...hit, note: null, reason: null };
      });
      return { product: server.product, version: server.version, database: server.database, checks, versions_snapshot: snapshot.date, versions_sources: ['https://www.postgresql.org/support/security/', 'https://www.sqlite.org/cves.html'] };
    },
  };
}
