# Compatible engines

TimescaleDB, YugabyteDB, Neon and Supabase speak the PostgreSQL protocol; TiDB and PlanetScale speak MySQL's. NiceDB treats them as engines of their own: mark, port, starting values, and only the tools that really work.

## Connecting
1. In the connection form pick the product among the **Engines**: it sits right after PostgreSQL or MySQL.
2. Paste the URL from the service's console into **Connection URL**: host, port, database and user fill in, the password goes to the keychain. Or fill the fields by hand; the placeholders show what they usually look like.
3. Press **Test**: the answer names the product and its version, for example “TimescaleDB 2.31.0 · sales”.

| Product | Port | Notes |
|---|---|---|
| TimescaleDB | 5432 | |
| YugabyteDB | 5433 | YSQL; user and database `yugabyte` |
| Neon | 5432 | TLS required; `-pooler` in the host goes through the pooler |
| Supabase | 5432, 6543 | TLS required; 6543 is the transaction pooler |
| TiDB | 4000 | |
| PlanetScale | 3306 | TLS required; one password per branch |

Connecting to one of these servers as plain PostgreSQL or MySQL, NiceDB tells it apart by itself from the version, the extensions or the host name.

## The engine card
In **Tools**, below the list, the engine card names the product and its version, the engine it sits on and what the service reveals: Neon's endpoint, region, branch and pooler, Supabase's project, the nodes of YugabyteDB and TiDB. **Worth knowing** lists the limits that change the work, for example that through the pooler session settings do not outlive the transaction.

## TimescaleDB
- Hypertables carry the **hyper** tag in the tree; the tooltip gives chunks and compressed chunks.
- **Continuous aggregates** have their own group; TimescaleDB's schemas sit together under **TimescaleDB**, the internal ones are hidden.
- **Show DDL** adds `create_hypertable`, compression and the compression and retention policies; a continuous aggregate comes out with its original query.
- **Tools › Disk usage** counts every chunk, shows what compression freed and lists the chunks with the **compressed** tag.

## Supabase and row level security
The platform's schemas (`auth`, `storage`, `realtime`, `vault`…) sit together under **Supabase**, apart from the project's: better leave them alone. On every PostgreSQL, tables with row level security on carry the **RLS** tag, and **Show DDL** also writes `ENABLE ROW LEVEL SECURITY` and the policies (`CREATE POLICY`).

## TiDB and PlanetScale
- TiDB: no procedures or triggers; **Explain** reads TiDB's plans, monitoring uses TiDB's tables for connections, transactions, lock waits and slow queries.
- PlanetScale: **Users and privileges**, **Backup and restore** and **Security** are not available, because they live in the service's dashboard. On the production branch the schema changes through deploy requests, and foreign keys hold only if the database has them turned on.

## Limits
- Neon, Supabase and PlanetScale are told apart by host name and server settings; behind a custom domain, pick them in the form.
- A TimescaleDB backup made with `pg_dump` must be restored with TimescaleDB's procedures.
- On YugabyteDB and TiDB locks are distributed: monitoring sees the waits, not the whole tree.
