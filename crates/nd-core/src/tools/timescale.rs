use nd_driver::{Connection, Row};
use serde::Serialize;

use super::PartitionSize;
use super::activity::{Sizes, cell, num, rows};

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct HypertableSize {
    pub namespace: String,
    pub name: String,
    pub chunks: Option<f64>,
    pub compressed_chunks: Option<f64>,
    pub before_compression: Option<f64>,
    pub after_compression: Option<f64>,
    pub total_bytes: Option<f64>,
}

const HYPERTABLES: &str = "SELECT h.hypertable_schema::text, h.hypertable_name::text, h.num_chunks::text, \
s.number_compressed_chunks::text, s.before_compression_total_bytes::text, s.after_compression_total_bytes::text, \
hypertable_size(format('%I.%I', h.hypertable_schema, h.hypertable_name)::regclass)::text \
FROM timescaledb_information.hypertables h \
LEFT JOIN LATERAL hypertable_compression_stats(format('%I.%I', h.hypertable_schema, h.hypertable_name)::regclass) s ON true \
WHERE h.hypertable_schema NOT LIKE '\\_timescaledb\\_%' ORDER BY 7 DESC NULLS LAST";

const CHUNKS: &str = "SELECT c.hypertable_schema::text, c.hypertable_name::text, \
coalesce(c.range_start::date::text || ' → ' || c.range_end::date::text, c.range_start_integer::text || ' → ' || c.range_end_integer::text), \
NULL::text, NULL::text, d.total_bytes::text, c.is_compressed::text \
FROM timescaledb_information.chunks c \
LEFT JOIN LATERAL chunks_detailed_size(format('%I.%I', c.hypertable_schema, c.hypertable_name)::regclass) d ON d.chunk_name = c.chunk_name \
WHERE c.hypertable_schema NOT LIKE '\\_timescaledb\\_%' \
ORDER BY 1, 2, c.range_start DESC NULLS LAST, c.range_start_integer DESC NULLS LAST LIMIT 1000";

fn hypertable_of(r: &Row) -> HypertableSize {
    HypertableSize {
        namespace: cell(r, 0).unwrap_or_default(),
        name: cell(r, 1).unwrap_or_default(),
        chunks: num(r, 2),
        compressed_chunks: num(r, 3),
        before_compression: num(r, 4),
        after_compression: num(r, 5),
        total_bytes: num(r, 6),
    }
}

pub(crate) async fn sizes(conn: &dyn Connection, sizes: &mut Sizes) {
    if conn
        .edition()
        .is_none_or(|e| e.flavor != nd_driver::Flavor::Timescale)
    {
        return;
    }
    let internal = |ns: &str| ns.starts_with("_timescaledb_");
    sizes.tables.retain(|t| !internal(&t.namespace));
    sizes.indexes.retain(|i| !internal(&i.namespace));
    if let Ok(found) = rows(conn, HYPERTABLES).await {
        sizes.hypertables = found.iter().map(hypertable_of).collect();
    }
    for h in &sizes.hypertables {
        if let Some(t) = sizes
            .tables
            .iter_mut()
            .find(|t| t.namespace == h.namespace && t.name == h.name)
        {
            t.total_bytes = h.total_bytes;
            t.data_bytes = None;
            t.index_bytes = None;
        }
    }
    sizes.tables.sort_by(|a, b| {
        b.total_bytes
            .unwrap_or(0.0)
            .total_cmp(&a.total_bytes.unwrap_or(0.0))
    });
    if let Ok(found) = rows(conn, CHUNKS).await {
        sizes.partitions = found
            .iter()
            .map(|r| PartitionSize {
                namespace: cell(r, 0).unwrap_or_default(),
                table: cell(r, 1).unwrap_or_default(),
                partition: cell(r, 2).unwrap_or_default(),
                parts: None,
                rows: num(r, 4),
                bytes: num(r, 5),
                compressed: cell(r, 6).map(|c| c == "true"),
            })
            .collect();
    }
}
