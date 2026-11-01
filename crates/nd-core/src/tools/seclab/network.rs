use std::collections::HashMap;

use super::{Ctx, Outcome, SecSeverity, SecSkip, cell, found, rows, scalar, subject};
use crate::tools::Engine;

async fn settings(cx: &Ctx<'_>, names: &[&str]) -> Option<HashMap<String, String>> {
    let list = names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = if cx.engine == Engine::Postgres {
        format!("SELECT name, setting FROM pg_settings WHERE name IN ({list})")
    } else {
        format!("SHOW VARIABLES WHERE Variable_name IN ({list})")
    };
    let r = rows(cx.conn, &sql).await?;
    Some(
        r.iter()
            .map(|row| (cell(row, 0).to_lowercase(), cell(row, 1)))
            .collect(),
    )
}

struct Rule {
    line: String,
    kind: String,
    address: String,
    mask: String,
    method: String,
    text: String,
}

async fn hba_rules(cx: &Ctx<'_>) -> Option<Vec<Rule>> {
    let r = rows(
        cx.conn,
        "SELECT line_number::text, type, database::text, user_name::text, coalesce(address, ''), coalesce(netmask, ''), \
coalesce(auth_method, '') FROM pg_hba_file_rules WHERE error IS NULL ORDER BY line_number",
    )
    .await?;
    Some(
        r.iter()
            .map(|row| Rule {
                line: cell(row, 0),
                kind: cell(row, 1),
                address: cell(row, 4),
                mask: cell(row, 5),
                method: cell(row, 6),
                text: format!(
                    "{} {} {} {} {}",
                    cell(row, 1),
                    cell(row, 2).trim_matches(['{', '}']),
                    cell(row, 3).trim_matches(['{', '}']),
                    cell(row, 4),
                    cell(row, 6)
                )
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            })
            .collect(),
    )
}

fn loopback(r: &Rule) -> bool {
    r.kind == "local" || r.address.starts_with("127.") || r.address == "::1"
}

fn anywhere(r: &Rule) -> bool {
    r.address == "all"
        || (r.mask == "0.0.0.0" && r.address == "0.0.0.0")
        || r.address.ends_with("/0")
}

pub(super) async fn hba(id: &str, cx: &Ctx<'_>) -> Outcome {
    let Some(rules) = hba_rules(cx).await else {
        return Outcome::Skip(SecSkip::Privilege);
    };
    let hits: Vec<(&Rule, SecSeverity)> = rules
        .iter()
        .filter_map(|r| match id {
            "hba_trust" if r.method == "trust" => Some((
                r,
                if loopback(r) {
                    SecSeverity::Medium
                } else {
                    SecSeverity::High
                },
            )),
            "hba_trust" if r.method == "password" => Some((r, SecSeverity::High)),
            "hba_md5" if r.method == "md5" => Some((r, SecSeverity::Medium)),
            "hba_open"
                if r.kind.starts_with("host")
                    && r.kind != "hostssl"
                    && anywhere(r)
                    && r.method != "reject" =>
            {
                Some((r, SecSeverity::Medium))
            }
            _ => None,
        })
        .collect();
    let Some(severity) = hits.iter().map(|(_, s)| *s).min() else {
        return Outcome::Passed;
    };
    let mut fix: Vec<String> = hits
        .iter()
        .map(|(r, _)| match id {
            "hba_open" => format!(
                "-- pg_hba.conf line {}: narrow the address and use hostssl\n-- was: {}",
                r.line, r.text
            ),
            _ => format!(
                "-- pg_hba.conf line {}: use scram-sha-256\n-- was: {}",
                r.line, r.text
            ),
        })
        .collect();
    if id == "hba_md5" {
        fix.push("ALTER SYSTEM SET password_encryption = 'scram-sha-256';\n-- then set the passwords again: ALTER ROLE name PASSWORD '...';".into());
    }
    fix.push("SELECT pg_reload_conf();".into());
    let subjects = hits
        .iter()
        .map(|(r, _)| subject(r.line.clone(), Some(r.text.clone())))
        .collect();
    found(severity, subjects, fix)
}

const CH_TLS: &str = "<!-- /etc/clickhouse-server/config.d/tls.xml -->
<clickhouse>
  <https_port>8443</https_port>
  <tcp_port_secure>9440</tcp_port_secure>
  <openSSL>
    <server>
      <certificateFile>/etc/clickhouse-server/server.crt</certificateFile>
      <privateKeyFile>/etc/clickhouse-server/server.key</privateKeyFile>
    </server>
  </openSSL>
</clickhouse>";

pub(super) async fn ssl(cx: &Ctx<'_>) -> Outcome {
    match cx.engine {
        Engine::Postgres => {
            let Some(v) = scalar(cx.conn, "SHOW ssl").await else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            if v == "on" {
                return Outcome::Passed;
            }
            found(
                SecSeverity::High,
                vec![subject("ssl", Some(v))],
                vec!["-- needs a certificate and a key (ssl_cert_file, ssl_key_file)\nALTER SYSTEM SET ssl = on;\nSELECT pg_reload_conf();".into()],
            )
        }
        Engine::Mysql | Engine::Mariadb => {
            let Some(s) = settings(cx, &["have_ssl", "require_secure_transport"]).await else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let have = s
                .get("have_ssl")
                .map_or("", String::as_str)
                .to_ascii_uppercase();
            if have == "DISABLED" || have == "NO" {
                return found(
                    SecSeverity::High,
                    vec![subject("have_ssl", Some(have))],
                    vec!["-- in the configuration file: ssl_cert, ssl_key, ssl_ca; then restart the server".into()],
                );
            }
            let required = s
                .get("require_secure_transport")
                .is_some_and(|v| matches!(v.to_ascii_uppercase().as_str(), "ON" | "1"));
            if required {
                return Outcome::Passed;
            }
            let set = if cx.engine == Engine::Mysql {
                "SET PERSIST"
            } else {
                "SET GLOBAL"
            };
            found(
                SecSeverity::Medium,
                vec![subject("require_secure_transport", Some("OFF".into()))],
                vec![format!("{set} require_secure_transport = ON;")],
            )
        }
        Engine::Clickhouse => {
            for port in ["https_port", "tcp_port_secure"] {
                if scalar(cx.conn, &format!("SELECT getServerPort('{port}')"))
                    .await
                    .is_some()
                {
                    return Outcome::Passed;
                }
            }
            found(
                SecSeverity::High,
                vec![subject("https_port", None)],
                vec![CH_TLS.into()],
            )
        }
        _ => {
            let Some(v) = scalar(cx.conn, "SELECT CAST(encrypt_option AS varchar(10)) FROM sys.dm_exec_connections WHERE session_id = @@SPID").await else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            if v.eq_ignore_ascii_case("true") {
                return Outcome::Passed;
            }
            found(
                SecSeverity::Medium,
                vec![subject("encrypt_option", Some(v))],
                vec!["-- Configuration Manager: Protocols for the server > Properties > Force Encryption = Yes, with a valid certificate".into()],
            )
        }
    }
}

pub(super) async fn listen_all(cx: &Ctx<'_>) -> Outcome {
    let (name, value) = if cx.engine == Engine::Postgres {
        (
            "listen_addresses",
            scalar(cx.conn, "SHOW listen_addresses").await,
        )
    } else {
        (
            "bind_address",
            scalar(cx.conn, "SELECT @@bind_address").await,
        )
    };
    let Some(v) = value else {
        return Outcome::Skip(SecSkip::NotDetectable);
    };
    let open = v
        .split(',')
        .map(str::trim)
        .any(|a| matches!(a, "*" | "0.0.0.0" | "::" | "" | "::0"));
    if !open {
        return Outcome::Passed;
    }
    let fix = if cx.engine == Engine::Postgres {
        "ALTER SYSTEM SET listen_addresses = 'localhost';\n-- needs a restart; for one network address: 'localhost,10.0.0.5'".to_owned()
    } else {
        "-- in the configuration file: bind-address = 127.0.0.1 (or the internal address), then restart".to_owned()
    };
    found(
        SecSeverity::Medium,
        vec![subject(
            name,
            Some(if v.is_empty() { "*".into() } else { v }),
        )],
        vec![fix],
    )
}

pub(super) async fn logging(cx: &Ctx<'_>) -> Outcome {
    match cx.engine {
        Engine::Postgres => {
            let Some(s) = settings(cx, &["log_connections", "log_disconnections"]).await else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let off: Vec<_> = ["log_connections", "log_disconnections"]
                .into_iter()
                .filter(|n| matches!(s.get(*n).map(String::as_str), Some("off" | "")))
                .collect();
            if off.is_empty() {
                return Outcome::Passed;
            }
            let mut fix: Vec<String> = off
                .iter()
                .map(|n| format!("ALTER SYSTEM SET {n} = on;"))
                .collect();
            fix.push("SELECT pg_reload_conf();".into());
            found(
                SecSeverity::Low,
                off.iter()
                    .map(|n| subject(*n, Some("off".into())))
                    .collect(),
                fix,
            )
        }
        Engine::Mysql => {
            let v = scalar(cx.conn, "SELECT @@log_error_verbosity")
                .await
                .and_then(|v| v.parse::<u8>().ok());
            match v {
                None => Outcome::Skip(SecSkip::Privilege),
                Some(n) if n >= 2 => Outcome::Passed,
                Some(n) => found(
                    SecSeverity::Low,
                    vec![subject("log_error_verbosity", Some(n.to_string()))],
                    vec!["SET PERSIST log_error_verbosity = 2;".into()],
                ),
            }
        }
        Engine::Mariadb => {
            let v = scalar(cx.conn, "SELECT @@log_warnings")
                .await
                .and_then(|v| v.parse::<u8>().ok());
            match v {
                None => Outcome::Skip(SecSkip::Privilege),
                Some(n) if n >= 2 => Outcome::Passed,
                Some(n) => found(
                    SecSeverity::Low,
                    vec![subject("log_warnings", Some(n.to_string()))],
                    vec!["SET GLOBAL log_warnings = 2;".into()],
                ),
            }
        }
        Engine::Clickhouse => {
            let Some(v) = scalar(
                cx.conn,
                "SELECT value FROM system.settings WHERE name = 'log_queries'",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            if v == "1" {
                return Outcome::Passed;
            }
            found(
                SecSeverity::Low,
                vec![subject("log_queries", Some(v))],
                vec!["ALTER SETTINGS PROFILE `default` SETTINGS log_queries = 1;".into()],
            )
        }
        _ => {
            let Some(r) = rows(cx.conn, "EXEC xp_loginconfig 'audit level'").await else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let level = r
                .first()
                .map(|row| cell(row, 1).to_lowercase())
                .unwrap_or_default();
            if level == "all" || level.is_empty() {
                return if level.is_empty() {
                    Outcome::Skip(SecSkip::Privilege)
                } else {
                    Outcome::Passed
                };
            }
            let severity = if level == "none" {
                SecSeverity::Medium
            } else {
                SecSeverity::Low
            };
            found(
                severity,
                vec![subject("audit level", Some(level))],
                vec!["EXEC xp_instance_regwrite N'HKEY_LOCAL_MACHINE', N'Software\\Microsoft\\MSSQLServer\\MSSQLServer', N'AuditLevel', REG_DWORD, 3;\n-- needs a restart of the service".into()],
            )
        }
    }
}

pub(super) async fn backup(cx: &Ctx<'_>) -> Outcome {
    let database = cx.conn.server().database.clone();
    match cx.engine {
        Engine::Mssql => {
            let Some(r) = rows(
                cx.conn,
                "SELECT DATEDIFF(day, MAX(backup_finish_date), GETDATE()) FROM msdb.dbo.backupset WHERE database_name = DB_NAME() AND type = 'D'",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let days = r
                .first()
                .map(|row| cell(row, 0))
                .and_then(|v| v.parse::<i64>().ok());
            if days.is_some_and(|d| d <= 7) {
                return Outcome::Passed;
            }
            let detail = days.map_or_else(|| "never".to_owned(), |d| d.to_string());
            found(
                SecSeverity::Medium,
                vec![subject(database.clone(), Some(detail))],
                vec![format!(
                    "BACKUP DATABASE {} TO DISK = N'/var/opt/mssql/data/{database}.bak' WITH COMPRESSION, CHECKSUM;",
                    cx.q(&database)
                )],
            )
        }
        Engine::Postgres => {
            let Some(r) = rows(
                cx.conn,
                "SELECT current_setting('archive_mode'), coalesce(extract(epoch FROM now() - last_archived_time)::bigint::text, '') FROM pg_stat_archiver",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let Some(row) = r.first() else {
                return Outcome::Skip(SecSkip::NotDetectable);
            };
            if cell(row, 0) == "off" {
                return Outcome::Skip(SecSkip::NotDetectable);
            }
            if cell(row, 1).parse::<i64>().is_ok_and(|s| s < 2 * 86_400) {
                return Outcome::Passed;
            }
            found(
                SecSeverity::Medium,
                vec![subject("archive_mode", Some("on".into()))],
                vec!["-- WAL archiving is on but has not archived for more than two days: check archive_command and the space at the destination".into()],
            )
        }
        Engine::Cockroach => {
            let Some(r) = rows(
                cx.conn,
                "SELECT coalesce(extract(epoch FROM now() - max(finished))::INT8::STRING, '') FROM [SHOW JOBS] WHERE job_type = 'BACKUP' AND status = 'succeeded'",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let secs = r
                .first()
                .map(|row| cell(row, 0))
                .and_then(|v| v.parse::<i64>().ok());
            if secs.is_some_and(|s| s <= 7 * 86_400) {
                return Outcome::Passed;
            }
            found(
                SecSeverity::Medium,
                vec![subject(database.clone(), None)],
                vec![format!(
                    "BACKUP DATABASE {} INTO 'nodelocal://1/backup';",
                    cx.q(&database)
                )],
            )
        }
        Engine::Clickhouse => clickhouse_backup(cx, &database).await,
        _ => Outcome::Skip(SecSkip::NotDetectable),
    }
}

async fn clickhouse_backup(cx: &Ctx<'_>, database: &str) -> Outcome {
    let Some(r) = rows(
        cx.conn,
        "SELECT countIf(status = 'BACKUP_CREATED'), \
         dateDiff('day', maxIf(event_time, status = 'BACKUP_CREATED'), now()) FROM system.backup_log",
    )
    .await
    else {
        return Outcome::Skip(SecSkip::NotDetectable);
    };
    let row = r.first();
    let made = row.is_some_and(|row| cell(row, 0) != "0");
    let days = row.and_then(|row| cell(row, 1).parse::<i64>().ok());
    if made && days.is_some_and(|d| d <= 7) {
        return Outcome::Passed;
    }
    let detail = match days {
        Some(d) if made => d.to_string(),
        _ => "never".to_owned(),
    };
    found(
        SecSeverity::Medium,
        vec![subject(database, Some(detail))],
        vec![format!(
            "BACKUP DATABASE {} TO File('{database}.zip');",
            cx.q(database)
        )],
    )
}

const CH_REACH: &str = "'FILE', 'URL', 'REMOTE', 'S3', 'HDFS', 'SOURCES', 'INTROSPECTION'";

const PG_RISKY: &[&str] = &[
    "adminpack",
    "file_fdw",
    "xml2",
    "plpython3u",
    "plpythonu",
    "plperlu",
    "pltclu",
    "plsh",
    "dblink",
    "postgres_fdw",
];
const PG_UNTRUSTED: &[&str] = &[
    "adminpack",
    "file_fdw",
    "xml2",
    "plpython3u",
    "plpythonu",
    "plperlu",
    "pltclu",
    "plsh",
];

#[allow(clippy::too_many_lines)]
pub(super) async fn risky(cx: &Ctx<'_>) -> Outcome {
    match cx.engine {
        Engine::Postgres => {
            let list = PG_RISKY
                .iter()
                .map(|n| format!("'{n}'"))
                .collect::<Vec<_>>()
                .join(", ");
            let Some(ext) = rows(
                cx.conn,
                &format!("SELECT extname FROM pg_extension WHERE extname IN ({list})"),
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let langs = rows(cx.conn, "SELECT lanname FROM pg_language WHERE NOT lanpltrusted AND lanname NOT IN ('c', 'internal')")
                .await
                .unwrap_or_default();
            let names: Vec<String> = ext.iter().map(|r| cell(r, 0)).collect();
            let mut subjects: Vec<_> = names
                .iter()
                .map(|n| subject(n.clone(), Some("extension".into())))
                .collect();
            let mut fix: Vec<String> = names
                .iter()
                .map(|n| format!("DROP EXTENSION {};", cx.q(n)))
                .collect();
            for l in langs
                .iter()
                .map(|r| cell(r, 0))
                .filter(|l| !names.contains(l))
            {
                fix.push(format!("DROP LANGUAGE {};", cx.q(&l)));
                subjects.push(subject(l, Some("language".into())));
            }
            if subjects.is_empty() {
                return Outcome::Passed;
            }
            let severity = if subjects.iter().any(|s| {
                PG_UNTRUSTED.contains(&s.name.as_str()) || s.detail.as_deref() == Some("language")
            }) {
                SecSeverity::Medium
            } else {
                SecSeverity::Low
            };
            found(severity, subjects, fix)
        }
        Engine::Mssql => {
            let Some(r) = rows(
                cx.conn,
                "SELECT name, CAST(value_in_use AS int) FROM sys.configurations WHERE value_in_use = 1 AND name IN \
('xp_cmdshell', 'Ole Automation Procedures', 'Ad Hoc Distributed Queries', 'clr enabled', 'external scripts enabled', 'cross db ownership chaining')",
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            if r.is_empty() {
                return Outcome::Passed;
            }
            let mut fix =
                vec!["EXEC sp_configure 'show advanced options', 1;\nRECONFIGURE;".to_owned()];
            fix.extend(
                r.iter()
                    .map(|row| format!("EXEC sp_configure '{}', 0;", cell(row, 0))),
            );
            fix.push("RECONFIGURE;".into());
            let severity = if r.iter().any(|row| cell(row, 0) == "xp_cmdshell") {
                SecSeverity::High
            } else {
                SecSeverity::Medium
            };
            found(
                severity,
                r.iter().map(|row| subject(cell(row, 0), None)).collect(),
                fix,
            )
        }
        Engine::Clickhouse => {
            let Some(r) = rows(
                cx.conn,
                &format!(
                    "SELECT ifNull(user_name, role_name), arrayStringConcat(groupArray(DISTINCT access_type), ', ') \
                     FROM system.grants WHERE access_type IN ({CH_REACH}) AND database IS NULL \
                     AND NOT is_partial_revoke GROUP BY 1 ORDER BY 1"
                ),
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let introspection = scalar(
                cx.conn,
                "SELECT value FROM system.settings WHERE name = 'allow_introspection_functions'",
            )
            .await
            .is_some_and(|v| v == "1");
            let mut subjects: Vec<_> = r
                .iter()
                .map(|row| subject(cell(row, 0), Some(cell(row, 1))))
                .collect();
            let mut fix: Vec<String> = r
                .iter()
                .map(|row| {
                    format!(
                        "REVOKE {} ON *.* FROM {};",
                        cell(row, 1),
                        cx.q(&cell(row, 0))
                    )
                })
                .collect();
            if introspection {
                subjects.push(subject("allow_introspection_functions", Some("1".into())));
                fix.push(
                    "ALTER SETTINGS PROFILE `default` SETTINGS allow_introspection_functions = 0;"
                        .into(),
                );
            }
            if subjects.is_empty() {
                return Outcome::Passed;
            }
            found(SecSeverity::Medium, subjects, fix)
        }
        _ => {
            let Some(s) = settings(
                cx,
                &["local_infile", "secure_file_priv", "skip_grant_tables"],
            )
            .await
            else {
                return Outcome::Skip(SecSkip::Privilege);
            };
            let mut subjects = Vec::new();
            let mut fix = Vec::new();
            let mut severity = SecSeverity::Medium;
            if s.get("local_infile")
                .is_some_and(|v| matches!(v.to_ascii_uppercase().as_str(), "ON" | "1"))
            {
                subjects.push(subject("local_infile", Some("ON".into())));
                fix.push("SET GLOBAL local_infile = 0;".to_owned());
            }
            if s.get("secure_file_priv").is_some_and(String::is_empty) {
                subjects.push(subject("secure_file_priv", Some(String::new())));
                fix.push("-- in the configuration file: secure_file_priv = /var/lib/mysql-files, then restart".into());
            }
            if s.get("skip_grant_tables").is_some_and(|v| on_value(v)) {
                severity = SecSeverity::High;
                subjects.push(subject("skip_grant_tables", Some("ON".into())));
                fix.push("-- remove skip-grant-tables from the configuration and restart: privileges are not checked".into());
            }
            if subjects.is_empty() {
                return Outcome::Passed;
            }
            found(severity, subjects, fix)
        }
    }
}

fn on_value(v: &str) -> bool {
    matches!(v.to_ascii_uppercase().as_str(), "ON" | "1" | "TRUE")
}
