use nd_diff::{Col, Idx, Snapshot, Table, View};
use nd_driver::{Connection, Dialect, Row};
use serde::Serialize;

use super::activity::{IndexUsage, Sizes, TableSize, cell, num, rows};
use super::monitor::{
    ConnectionCount, DatabaseSize, GapReason, Monitor, MonitorGap, MonitorPart, SlowQueries,
    SlowQuery, SlowSource,
};

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct PartitionSize {
    pub namespace: String,
    pub table: String,
    pub partition: String,
    pub parts: Option<f64>,
    pub rows: Option<f64>,
    pub bytes: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub compressed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct BackgroundMerge {
    pub namespace: String,
    pub table: String,
    pub elapsed_ms: Option<f64>,
    pub progress: Option<f64>,
    pub parts: Option<f64>,
    pub bytes: Option<f64>,
    pub mutation: bool,
}

const USER_DATABASES: &str =
    "database NOT IN ('system', 'INFORMATION_SCHEMA', 'information_schema')";

pub(crate) fn sessions_sql() -> String {
    "SELECT query_id, user, current_database, toString(address), client_name, \
     if(is_cancelled, 'cancelled', 'active'), query, elapsed * 1000, NULL \
     FROM system.processes WHERE query_id <> queryID() ORDER BY elapsed DESC"
        .into()
}

pub(crate) fn kill_sql(id: &str) -> String {
    format!(
        "KILL QUERY WHERE query_id = {} ASYNC",
        nd_sql::literal(nd_sql::Dialect::Clickhouse, Some(id))
    )
}

pub(crate) async fn sizes(
    conn: &dyn Connection,
) -> std::result::Result<Sizes, nd_driver::DriverError> {
    let tables = rows(
        conn,
        &format!(
            "SELECT database, table, sum(rows), sum(data_compressed_bytes), sum(primary_key_bytes_in_memory), \
             sum(bytes_on_disk) FROM system.parts WHERE active AND {USER_DATABASES} \
             GROUP BY database, table ORDER BY sum(bytes_on_disk) DESC"
        ),
    )
    .await?;
    let indexes = rows(
        conn,
        &format!(
            "SELECT database, table, name, data_compressed_bytes FROM system.data_skipping_indices \
             WHERE {USER_DATABASES} ORDER BY database, table, name"
        ),
    )
    .await
    .unwrap_or_default();
    let partitions = rows(
        conn,
        &format!(
            "SELECT database, table, partition, count(), sum(rows), sum(bytes_on_disk) FROM system.parts \
             WHERE active AND {USER_DATABASES} GROUP BY database, table, partition \
             ORDER BY database, table, partition"
        ),
    )
    .await
    .unwrap_or_default();
    Ok(Sizes {
        tables: tables
            .iter()
            .map(|r| TableSize {
                namespace: cell(r, 0).unwrap_or_default(),
                name: cell(r, 1).unwrap_or_default(),
                rows: num(r, 2),
                data_bytes: num(r, 3),
                index_bytes: num(r, 4),
                total_bytes: num(r, 5),
            })
            .collect(),
        indexes: indexes
            .iter()
            .map(|r| IndexUsage {
                namespace: cell(r, 0).unwrap_or_default(),
                table: cell(r, 1).unwrap_or_default(),
                name: cell(r, 2).unwrap_or_default(),
                bytes: num(r, 3),
                scans: None,
                unique: false,
                unused: false,
            })
            .collect(),
        usage_known: false,
        partitions: partitions.iter().map(partition_of).collect(),
        hypertables: Vec::new(),
    })
}

fn partition_of(r: &Row) -> PartitionSize {
    PartitionSize {
        namespace: cell(r, 0).unwrap_or_default(),
        table: cell(r, 1).unwrap_or_default(),
        partition: cell(r, 2).unwrap_or_default(),
        parts: num(r, 3),
        rows: num(r, 4),
        bytes: num(r, 5),
        compressed: None,
    }
}

const CONNECTIONS: &str = "SELECT \
    (SELECT sum(value) FROM system.metrics WHERE metric IN ('TCPConnection', 'HTTPConnection', \
     'MySQLConnection', 'PostgreSQLConnection', 'InterserverConnection')), \
    (SELECT count() FROM system.processes WHERE query_id <> queryID()), \
    (SELECT toUInt64OrNull(value) FROM system.server_settings WHERE name = 'max_connections')";
const DATABASES: &str = "SELECT database, sum(bytes_on_disk) FROM system.parts WHERE active \
    GROUP BY database ORDER BY database";
const CACHE: &str = "SELECT sumIf(value, event = 'MarkCacheHits') \
    / nullIf(sumIf(value, event IN ('MarkCacheHits', 'MarkCacheMisses')), 0) FROM system.events";
const MERGES: &str = "SELECT database, table, elapsed * 1000, progress, num_parts, \
    total_size_bytes_compressed, is_mutation FROM system.merges ORDER BY elapsed DESC";

fn gap(m: &mut Monitor, part: MonitorPart, e: &nd_driver::DriverError) {
    m.gaps.push(MonitorGap {
        part,
        reason: GapReason::Failed {
            message: e.to_string(),
        },
    });
}

pub(crate) async fn monitor(conn: &dyn Connection) -> Monitor {
    let mut m = Monitor::default();
    match rows(conn, CONNECTIONS).await {
        Ok(found) => {
            let r = found.into_iter().next().unwrap_or_default();
            m.connections = Some(ConnectionCount {
                total: num(&r, 0),
                active: num(&r, 1),
                idle_in_transaction: None,
                waiting: None,
                max: num(&r, 2),
            });
        }
        Err(e) => gap(&mut m, MonitorPart::Connections, &e),
    }
    m.gaps.push(MonitorGap {
        part: MonitorPart::Transactions,
        reason: GapReason::NotExposed,
    });
    match rows(conn, DATABASES).await {
        Ok(found) => {
            m.databases = found
                .iter()
                .map(|r| DatabaseSize {
                    name: cell(r, 0).unwrap_or_default(),
                    bytes: num(r, 1),
                })
                .collect();
        }
        Err(e) => gap(&mut m, MonitorPart::Databases, &e),
    }
    match rows(conn, CACHE).await {
        Ok(found) => m.cache_hit = found.first().and_then(|r| num(r, 0)),
        Err(e) => gap(&mut m, MonitorPart::CacheHit, &e),
    }
    m.merges = rows(conn, MERGES)
        .await
        .unwrap_or_default()
        .iter()
        .map(|r| BackgroundMerge {
            namespace: cell(r, 0).unwrap_or_default(),
            table: cell(r, 1).unwrap_or_default(),
            elapsed_ms: num(r, 2),
            progress: num(r, 3),
            parts: num(r, 4),
            bytes: num(r, 5),
            mutation: cell(r, 6).as_deref() == Some("1"),
        })
        .collect();
    m
}

const SLOW: &str = "SELECT any(normalizeQuery(query)), any(query), count(), sum(query_duration_ms), \
    avg(query_duration_ms), sum(result_rows) FROM system.query_log \
    WHERE type = 'QueryFinish' AND event_time > now() - INTERVAL 1 DAY AND is_initial_query \
    AND NOT arrayAll(d -> d IN ('system', 'INFORMATION_SCHEMA', 'information_schema'), databases) \
    GROUP BY normalized_query_hash ORDER BY sum(query_duration_ms) DESC LIMIT 50";

pub(crate) async fn slow(conn: &dyn Connection) -> SlowQueries {
    match rows(conn, SLOW).await {
        Ok(found) => SlowQueries {
            source: Some(SlowSource::QueryLog),
            gap: None,
            queries: found
                .iter()
                .map(|r| SlowQuery {
                    text: cell(r, 0).unwrap_or_default().trim().to_owned(),
                    sample: cell(r, 1).map(|s| s.trim().to_owned()),
                    calls: num(r, 2),
                    total_ms: num(r, 3),
                    mean_ms: num(r, 4),
                    rows: num(r, 5),
                })
                .collect(),
        },
        Err(e) => SlowQueries {
            source: None,
            gap: Some(GapReason::Failed {
                message: e.to_string(),
            }),
            queries: Vec::new(),
        },
    }
}

#[derive(Debug, Clone, Default, Serialize, specta::Type)]
pub struct ClickhouseAccess {
    pub profiles: Vec<SettingsProfile>,
    pub policies: Vec<RowPolicy>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct SettingsProfile {
    pub name: String,
    pub all: bool,
    pub applies_to: Vec<String>,
    pub except: Vec<String>,
    pub settings: Vec<ProfileSetting>,
    pub inherits: Vec<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ProfileSetting {
    pub name: String,
    pub value: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
    pub writability: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct RowPolicy {
    pub name: String,
    pub database: String,
    pub table: String,
    pub filter: Option<String>,
    pub restrictive: bool,
    pub all: bool,
    pub applies_to: Vec<String>,
    pub except: Vec<String>,
}

const CH_PROFILES: &str = "SELECT name, apply_to_all, arrayStringConcat(apply_to_list, ','), \
arrayStringConcat(apply_to_except, ',') FROM system.settings_profiles ORDER BY name";
const CH_PROFILE_ELEMENTS: &str = "SELECT profile_name, setting_name, value, min, max, \
toString(writability), inherit_profile FROM system.settings_profile_elements \
WHERE profile_name IS NOT NULL ORDER BY profile_name, index";
const CH_ROW_POLICIES: &str = "SELECT short_name, database, table, select_filter, is_restrictive, \
apply_to_all, arrayStringConcat(apply_to_list, ','), arrayStringConcat(apply_to_except, ',') \
FROM system.row_policies ORDER BY database, table, short_name";

fn yes(r: &Row, i: usize) -> bool {
    matches!(cell(r, i).as_deref(), Some("1" | "true"))
}

fn names(r: &Row, i: usize) -> Vec<String> {
    cell(r, i)
        .filter(|s| !s.is_empty())
        .map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default()
}

pub(crate) fn access_of(profiles: &[Row], elements: &[Row], policies: &[Row]) -> ClickhouseAccess {
    let profiles = profiles
        .iter()
        .map(|r| {
            let name = cell(r, 0).unwrap_or_default();
            let mine = elements
                .iter()
                .filter(|e| cell(e, 0).as_deref() == Some(name.as_str()));
            let mut settings = Vec::new();
            let mut inherits = Vec::new();
            for e in mine {
                if let Some(setting) = cell(e, 1) {
                    settings.push(ProfileSetting {
                        name: setting,
                        value: cell(e, 2),
                        min: cell(e, 3),
                        max: cell(e, 4),
                        writability: cell(e, 5).filter(|w| !w.is_empty() && w != "\\N"),
                    });
                } else if let Some(parent) = cell(e, 6) {
                    inherits.push(parent);
                }
            }
            SettingsProfile {
                name,
                all: yes(r, 1),
                applies_to: names(r, 2),
                except: names(r, 3),
                settings,
                inherits,
            }
        })
        .collect();
    let policies = policies
        .iter()
        .map(|r| RowPolicy {
            name: cell(r, 0).unwrap_or_default(),
            database: cell(r, 1).unwrap_or_default(),
            table: cell(r, 2).unwrap_or_default(),
            filter: cell(r, 3),
            restrictive: yes(r, 4),
            all: yes(r, 5),
            applies_to: names(r, 6),
            except: names(r, 7),
        })
        .collect();
    ClickhouseAccess { profiles, policies }
}

pub(crate) async fn access(conn: &dyn Connection) -> ClickhouseAccess {
    let read = async |sql: &str| rows(conn, sql).await.unwrap_or_default();
    access_of(
        &read(CH_PROFILES).await,
        &read(CH_PROFILE_ELEMENTS).await,
        &read(CH_ROW_POLICIES).await,
    )
}

pub(crate) async fn snapshot(
    conn: &dyn Connection,
    database: &str,
) -> std::result::Result<Snapshot, nd_driver::DriverError> {
    let db = nd_sql::literal(Dialect::Clickhouse, Some(database));
    let tables = rows(
        conn,
        &format!(
            "SELECT name, engine, create_table_query, as_select, primary_key FROM system.tables \
             WHERE database = {db} AND NOT is_temporary ORDER BY name"
        ),
    )
    .await?;
    let columns = rows(
        conn,
        &format!(
            "SELECT table, name, type, default_kind, default_expression FROM system.columns \
             WHERE database = {db} ORDER BY table, position"
        ),
    )
    .await?;
    let skipping = rows(
        conn,
        &format!(
            "SELECT table, name, expr, type_full, granularity FROM system.data_skipping_indices \
             WHERE database = {db} ORDER BY table, name"
        ),
    )
    .await
    .unwrap_or_default();
    let text = |r: &Row, i: usize| cell(r, i).unwrap_or_default();
    let local = |def: &str| super::compare::local(def, database, Dialect::Clickhouse);
    let mut out = Snapshot {
        namespace: database.to_owned(),
        ..Snapshot::default()
    };
    for r in &tables {
        let name = text(r, 0);
        match text(r, 1).as_str() {
            "View" => out.views.push(View {
                definition: cell(r, 3).filter(|s| !s.is_empty()).map(|s| local(&s)),
                name,
            }),
            "MaterializedView" => out.views.push(View {
                definition: cell(r, 2).filter(|s| !s.is_empty()).map(|s| local(&s)),
                name,
            }),
            "Dictionary" => {}
            _ => {
                let columns = columns
                    .iter()
                    .filter(|c| text(c, 0) == name)
                    .map(|c| {
                        let (kind, expr) = (text(c, 3), text(c, 4));
                        Col {
                            name: text(c, 1),
                            type_name: text(c, 2),
                            nullable: true,
                            default: match kind.as_str() {
                                "" => None,
                                "DEFAULT" => Some(expr),
                                "EPHEMERAL" if expr.is_empty() => Some("EPHEMERAL".to_owned()),
                                _ => Some(format!("{kind} {expr}")),
                            },
                            generated: false,
                            default_name: None,
                        }
                    })
                    .collect();
                let indexes = skipping
                    .iter()
                    .filter(|i| text(i, 0) == name)
                    .map(|i| Idx {
                        name: text(i, 1),
                        columns: vec![format!(
                            "{} TYPE {} GRANULARITY {}",
                            text(i, 2),
                            text(i, 3),
                            text(i, 4)
                        )],
                        unique: false,
                    })
                    .collect();
                out.tables.push(Table {
                    columns,
                    primary_key: nd_drv_clickhouse::key_parts(&text(r, 4)),
                    pk_name: None,
                    indexes,
                    foreign_keys: Vec::new(),
                    create: cell(r, 2).filter(|s| !s.is_empty()).map(|s| local(&s)),
                    name,
                });
            }
        }
    }
    Ok(out)
}
