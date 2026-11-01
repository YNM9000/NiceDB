# ClickHouse

ClickHouse works like the other SQL engines, with its own dialect. Rows can be read and analysed; they are not edited one at a time.

## Connect
1. **New connection**, then choose ClickHouse in **Engine**.
2. **Host** and **Port**: NiceDB uses the HTTP interface, 8123 in the clear and 8443 with TLS. Until you change it, the port follows the TLS choice. The native port 9000 is not used.
3. **User**: empty means `default`. The password is kept in the keychain.
4. **Database**: the one queries work on; empty means `default`.
5. **TLS**: **Verify certificate** uses HTTPS with the system certificates or the **CA certificate (optional)**; **Encrypted, unverified** accepts any certificate.
6. **Test**, then **Save**.

SSH tunnel and proxy work as for other servers; through the tunnel the certificate is checked against the host name typed in the form. **Copy as URL** in the connection's right-click menu gives `http://user@host:8123/?database=…`.

## Session
Each tab has a server session that lasts 30 minutes of inactivity. Settings made with `SET` and temporary tables persist between statements. The connection's init queries run there, for example:

```sql
SET max_threads = 4;
SET session_timezone = 'Europe/Rome';
```

## Explore
Under the connection are the databases; each holds:
- **Tables**, with their row count. Hovering a table shows its engine (`MergeTree`, `ReplacingMergeTree`…). Opening it shows columns, the highlighted primary key, **ORDER BY**, **PRIMARY KEY**, **PARTITION BY**, **SAMPLE BY** and the skip indexes.
- **Views** and **materialized views**, with the table they write to in the tooltip.
- **Dictionaries**, with loaded elements, layout and status.
- User-defined **functions**, if any.

Under the server are the active sessions.

Right-click a table for **Show DDL**, **Copy CREATE**, **Generate SQL**, **Dependencies…**, export and import, and **Rebuild the table** (`OPTIMIZE TABLE … FINAL`, which goes through the production check).

## Write queries
- Autocomplete offers tables, columns, types (`UInt32`, `LowCardinality`…) and ClickHouse functions with the right case (`toStartOfMonth`, `uniqExact`), because case matters in ClickHouse.
- [[key:format]] follows ClickHouse's rules.
- `:name` and `${year}` parameters work as elsewhere: see [Query parameters](parameters).
- A statement with its own `FORMAT` (`SELECT … FORMAT CSV`) arrives as text, one server line per grid row.
- A syntax error is underlined where the server points; the message carries the error name (`SYNTAX_ERROR`, `UNKNOWN_TABLE`…).

## Follow a long query
Rows arrive while the server produces them: the first at once, then in batches. 64-bit integers and decimals arrive exact; arrays and maps as JSON.

When a query takes more than a moment, progress appears above the results, for example “Read 1.4M of 3.2M rows”, with bytes read, percentage and a bar. If the server does not know how many rows it will read, the bar moves without a percentage.

[[key:cancel]] stops reading and stops the query on the server too.

## Data view
The **Data** view browses tables, views, materialized views and dictionaries with filters, sorting, search, pages and counts. It is read-only and says so at the top: “Read only: ClickHouse does not change rows one at a time.”. To change data, write `INSERT`, `ALTER TABLE … UPDATE` or `ALTER TABLE … DELETE` in the editor.

## Writes and production
- `SELECT`, `SHOW`, `DESCRIBE`, `EXPLAIN` and `EXISTS` are reads; `SELECT … INTO OUTFILE` counts as a write.
- `INSERT`, `ALTER`, `CREATE`, `DROP`, `TRUNCATE`, `OPTIMIZE`, `KILL`, `SYSTEM` and any statement not recognised as a read go through the production check and into the write log ([Production safety](production)).
- `DROP`, `TRUNCATE` and `ALTER`, mutations included, ask for confirmation as risky.
- After an `INSERT` the message says how many rows were written.

**Generate SQL** writes ClickHouse's forms: update as `ALTER TABLE … UPDATE … WHERE`, delete as `DELETE FROM … WHERE`, “upsert” as a plain `INSERT` (with `ReplacingMergeTree` the last row inserted per key wins), copy as `CREATE TABLE copy AS table` plus `INSERT … SELECT`.

## Query plan
**Explain** draws the plan as for other engines ([ER diagram and query plans](er-plans)). For each index used you see how many parts and granules remain: “parts 1/7 · granules 77/394”. The plan is always estimated: ClickHouse does not measure it by running the query.

Open the pipeline from the command palette with “Open the query pipeline (EXPLAIN PIPELINE)”: a new tab with `EXPLAIN PIPELINE` of the selected query.

## Tools
- **Activity**: running queries with user, client, database, duration and text. **Cancel query** sends `KILL QUERY`; on production it needs writes unlocked.
- **Monitor**: open connections and active queries, running **Merges** with mutations, space per database, the **Merges and mutations running** table and the last 24 hours' slow queries grouped by shape; **Explain** opens the plan of the sample.
- **Disk usage**: per table rows, compressed data, primary key in memory and disk; below, the partitions. More than 100 parts in a partition are flagged: it usually means inserts that are too small.
- **Import and export** as for other engines ([Import and export](import-export)); tables created by an import use `Nullable(…)` types.
- **Users and permissions**: users, roles and grants; for the chosen user or role the **Quotas**, the **Settings profiles** (with minimums, maximums, `CONST` and inherited profiles) and the **Row policies** with their condition. **New quota**, **New profile** and **New policy**, and the pencil or bin on each row, draft the SQL (`CREATE QUOTA`, `ALTER SETTINGS PROFILE`, `DROP ROW POLICY`…): you edit it and run it as from the editor, with the production check and the log.

**Find a value** and **Data compare** work here too. There is no ER diagram: ClickHouse has no foreign keys.
- **Schema compare**: compares the chosen database of two connections (the header dropdown decides which). The script creates new tables with the server's own `CREATE`, `ENGINE` included, adds, changes and drops columns and skip indexes with `ALTER TABLE`, and recreates views and materialized views. The primary key cannot change in place: the difference carries the "rebuild" note. Engine, partitions and dictionaries are not compared.
- **Test data**: with no transactions, rows go in batches of 1000. If the server refuses a batch the run stops and says which rows and why; the batches already written stay, with the count per table.
- **Backup**: `BACKUP DATABASE … TO File('name.zip')`, written by the server in its backups folder; you only choose the file name. Restore from the editor with `RESTORE DATABASE … FROM File('name.zip')`.
- **Security**: superusers, the `default` user, empty or plain-text passwords, DDL rights, TLS port, sensitive columns, query log, recent backups, `FILE`, `URL`, `S3` and similar rights.

## Limitations
- HTTP and HTTPS only: no native port, no gRPC. No TLS client certificate.
- No transactions from the editor, commit mode or dry run.
- A query the server stops halfway (memory, timeout) arrives as an error after the rows already received, which stay in the grid.
- Without the privilege to read `system.processes` the query runs without a progress bar.
- `USE other_db` applies to that statement only: switch database with the schema drop-down.
- The plan's estimated rows are a ceiling (granules × 8192), not a count.
