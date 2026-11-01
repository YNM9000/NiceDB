use std::fmt::Write as _;

use nd_driver::{DdlKind, Dialect, Flavor, ObjectRef, RelationTag, Schema, ident};
use tokio_postgres::Client;

const HYPERTABLES: &str = "SELECT h.hypertable_schema::text, h.hypertable_name::text, h.num_chunks::int4,
        (SELECT count(*)::int4 FROM timescaledb_information.chunks c
          WHERE c.hypertable_schema = h.hypertable_schema AND c.hypertable_name = h.hypertable_name AND c.is_compressed),
        approximate_row_count(format('%I.%I', h.hypertable_schema, h.hypertable_name)::regclass)::float8
     FROM timescaledb_information.hypertables h";

const AGGREGATES: &str =
    "SELECT view_schema::text, view_name::text FROM timescaledb_information.continuous_aggregates";

pub(crate) async fn enrich(client: &Client, schema: &mut Schema) {
    schema
        .namespaces
        .retain(|n| !n.name.starts_with("_timescaledb_"));
    for n in &mut schema.namespaces {
        if n.name.starts_with("timescaledb_") {
            n.managed = Some(Flavor::Timescale);
        }
    }
    match client.query(HYPERTABLES, &[]).await {
        Ok(rows) => {
            for r in rows {
                let (ns, name): (String, String) = (r.get(0), r.get(1));
                let Some(n) = schema.namespaces.iter_mut().find(|n| n.name == ns) else {
                    continue;
                };
                if let Some(t) = n.tables.iter_mut().find(|t| t.name == name) {
                    t.estimated_rows = r.get::<_, Option<f64>>(4).or(t.estimated_rows);
                }
                let count = |i: usize| u32::try_from(r.get::<_, i32>(i)).unwrap_or(0);
                n.tags.entry(name).or_default().insert(
                    0,
                    RelationTag::Hypertable {
                        chunks: count(2),
                        compressed: count(3),
                    },
                );
            }
        }
        Err(e) => tracing::debug!(error = %e, "timescale hypertables unreadable"),
    }
    match client.query(AGGREGATES, &[]).await {
        Ok(rows) => {
            for r in rows {
                let (ns, name): (String, String) = (r.get(0), r.get(1));
                let Some(n) = schema.namespaces.iter_mut().find(|n| n.name == ns) else {
                    continue;
                };
                if let Some(i) = n.views.iter().position(|v| v.name == name) {
                    let view = n.views.remove(i);
                    n.materialized_views.push(view);
                    n.tags
                        .entry(name)
                        .or_default()
                        .push(RelationTag::ContinuousAggregate);
                }
            }
        }
        Err(e) => tracing::debug!(error = %e, "timescale aggregates unreadable"),
    }
}

fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn span(value: &str) -> String {
    if value.parse::<i64>().is_ok() {
        value.to_owned()
    } else {
        format!("INTERVAL {}", literal(value))
    }
}

pub(crate) async fn ddl(client: &Client, object: &ObjectRef, plain: String) -> String {
    let made = match object.kind {
        DdlKind::Table => hypertable(client, object, &plain).await,
        DdlKind::View => aggregate(client, object).await,
        _ => Ok(None),
    };
    match made {
        Ok(Some(text)) => text,
        Ok(None) => plain,
        Err(e) => {
            tracing::debug!(error = %e, "timescale ddl unreadable");
            plain
        }
    }
}

async fn qualified(client: &Client, object: &ObjectRef) -> String {
    client
        .query_one(
            "SELECT quote_ident($1) || '.' || quote_ident($2)",
            &[&object.namespace, &object.name],
        )
        .await
        .map_or_else(
            |_| {
                format!(
                    "{}.{}",
                    ident(Dialect::Postgres, &object.namespace),
                    ident(Dialect::Postgres, &object.name)
                )
            },
            |r| r.get(0),
        )
}

async fn hypertable(
    client: &Client,
    object: &ObjectRef,
    plain: &str,
) -> Result<Option<String>, tokio_postgres::Error> {
    let (ns, name) = (&object.namespace, &object.name);
    let dims = client
        .query(
            "SELECT column_name::text, dimension_type, time_interval::text, integer_interval::text, num_partitions::int4
             FROM timescaledb_information.dimensions WHERE hypertable_schema = $1 AND hypertable_name = $2
             ORDER BY dimension_number",
            &[ns, name],
        )
        .await?;
    let Some(first) = dims.first() else {
        return Ok(None);
    };
    let q = qualified(client, object).await;
    let lit = literal(&q);
    let mut extra = String::new();
    let column: String = first.get(0);
    let interval = first
        .get::<_, Option<String>>(2)
        .or_else(|| first.get::<_, Option<String>>(3));
    let _ = write!(
        extra,
        "\n\nSELECT create_hypertable({lit}, {}",
        literal(&column)
    );
    if let Some(i) = interval {
        let _ = write!(extra, ", chunk_time_interval => {}", span(&i));
    }
    extra.push_str(", create_default_indexes => false);");
    for d in dims.iter().skip(1) {
        let column: String = d.get(0);
        if let Some(n) = d.get::<_, Option<i32>>(4) {
            let _ = write!(
                extra,
                "\nSELECT add_dimension({lit}, {}, number_partitions => {n});",
                literal(&column)
            );
        } else {
            let i = d
                .get::<_, Option<String>>(2)
                .or_else(|| d.get::<_, Option<String>>(3))
                .unwrap_or_default();
            let _ = write!(
                extra,
                "\nSELECT add_dimension({lit}, {}, chunk_time_interval => {});",
                literal(&column),
                span(&i)
            );
        }
    }
    storage(client, object, &q, &mut extra).await?;
    let cut = plain.find(';').map_or(plain.len(), |i| i + 1);
    Ok(Some(format!("{}{extra}{}", &plain[..cut], &plain[cut..])))
}

async fn storage(
    client: &Client,
    object: &ObjectRef,
    q: &str,
    extra: &mut String,
) -> Result<(), tokio_postgres::Error> {
    let (ns, name) = (&object.namespace, &object.name);
    let lit = literal(q);
    let compressed = client
        .query_opt(
            "SELECT segmentby, orderby FROM timescaledb_information.hypertable_compression_settings WHERE hypertable = $1::text::regclass",
            &[&q.to_owned()],
        )
        .await
        .ok()
        .flatten();
    if let Some(c) = compressed {
        let mut opts = vec!["timescaledb.compress".to_owned()];
        if let Some(s) = c.get::<_, Option<String>>(0).filter(|s| !s.is_empty()) {
            opts.push(format!("timescaledb.compress_segmentby = {}", literal(&s)));
        }
        if let Some(o) = c.get::<_, Option<String>>(1).filter(|s| !s.is_empty()) {
            opts.push(format!("timescaledb.compress_orderby = {}", literal(&o)));
        }
        let _ = write!(extra, "\n\nALTER TABLE {q} SET ({});", opts.join(", "));
    }
    let jobs = client
        .query(
            "SELECT proc_name::text, config->>'compress_after', config->>'drop_after'
             FROM timescaledb_information.jobs WHERE hypertable_schema = $1 AND hypertable_name = $2 ORDER BY job_id",
            &[ns, name],
        )
        .await?;
    for j in jobs {
        let proc: String = j.get(0);
        match (
            proc.as_str(),
            j.get::<_, Option<String>>(1),
            j.get::<_, Option<String>>(2),
        ) {
            ("policy_compression", Some(after), _) => {
                let _ = write!(
                    extra,
                    "\nSELECT add_compression_policy({lit}, {});",
                    span(&after)
                );
            }
            ("policy_retention", _, Some(after)) => {
                let _ = write!(
                    extra,
                    "\nSELECT add_retention_policy({lit}, {});",
                    span(&after)
                );
            }
            _ => {}
        }
    }
    Ok(())
}

async fn aggregate(
    client: &Client,
    object: &ObjectRef,
) -> Result<Option<String>, tokio_postgres::Error> {
    let Some(r) = client
        .query_opt(
            "SELECT view_definition, materialized_only, materialization_hypertable_schema::text, materialization_hypertable_name::text
             FROM timescaledb_information.continuous_aggregates WHERE view_schema = $1 AND view_name = $2",
            &[&object.namespace, &object.name],
        )
        .await?
    else {
        return Ok(None);
    };
    let q = qualified(client, object).await;
    let definition: String = r.get(0);
    let only: bool = r.get(1);
    let mut out = format!(
        "CREATE MATERIALIZED VIEW {q}\n    WITH (timescaledb.continuous, timescaledb.materialized_only = {only}) AS\n{}\nWITH NO DATA;",
        definition.trim().trim_end_matches(';')
    );
    let (mat_ns, mat_name): (String, String) = (r.get(2), r.get(3));
    let policy = client
        .query_opt(
            "SELECT config->>'start_offset', config->>'end_offset', schedule_interval::text
             FROM timescaledb_information.jobs
             WHERE proc_name = 'policy_refresh_continuous_aggregate' AND hypertable_schema = $1 AND hypertable_name = $2",
            &[&mat_ns, &mat_name],
        )
        .await?;
    if let Some(p) = policy {
        let offset = |v: Option<String>| v.map_or_else(|| "NULL".to_owned(), |v| span(&v));
        let schedule: String = p.get(2);
        let _ = write!(
            out,
            "\n\nSELECT add_continuous_aggregate_policy({}, start_offset => {}, end_offset => {}, schedule_interval => {});",
            literal(&q),
            offset(p.get(0)),
            offset(p.get(1)),
            span(&schedule)
        );
    }
    Ok(Some(out))
}
