# Cassandra and ScyllaDB

Cassandra and ScyllaDB keep rows in partitions, spread over the cluster's nodes by key. NiceDB reads them in CQL and shows them as documents, one row per document.

## Connecting
1. **New connection**, then pick Cassandra under **Engine** (ScyllaDB too: NiceDB tells the product by itself).
2. **Nodes**: one or more hosts separated by commas, all on the given **Port** (9042). With a single node NiceDB always goes through it, even when the cluster announces other addresses, so a node behind NAT or in a container works too.
3. **Keyspace**: the one queries work in; empty for none.
4. An empty **User** means a cluster without authentication; the password sits in the keychain.
5. **Consistency**: how many replicas must answer each read and write, such as `LOCAL_QUORUM` (the default) or `ONE`.
6. **Local datacenter**: with a cluster over several datacenters, the one to prefer.
7. TLS, client certificate, SSH tunnel and proxy work as for the other servers.

## Exploring
Under the connection are the keyspaces; in each, **Tables**, **Materialized views** and **Types**. Hovering a table shows its primary key, such as `PRIMARY KEY ((customer, year), number DESC)`: the partition key between the double parentheses, then the clustering columns with their order. Opening it shows the columns in that order, the key highlighted, and the secondary indexes.

A click on a table opens `SELECT * FROM table LIMIT 100;` and runs it. **Show DDL** asks the server for its `DESCRIBE`.

## Writing CQL
- Statements end with `;`. A `BEGIN BATCH … APPLY BATCH` stays one statement even with `;` inside.
- Completion offers keywords, types, functions, keyspaces, tables and columns.
- `CONSISTENCY QUORUM` changes the tab's consistency until you close it; `CONSISTENCY` alone tells the one in use.
- Names with capitals or reserved words need double quotes (`"Orders"`, `"token"`): completion, a click on the table and the quick snippets add them by themselves, only where needed.
- The quick snippets of the **Notes** panel are the ones Cassandra accepts: `tx` writes a `BEGIN BATCH … APPLY BATCH`, `lastn` a `PER PARTITION LIMIT`, `topn` asks for the partition before `ORDER BY`; those CQL lacks (joins, paging with `OFFSET`) are not listed.
- Rows arrive in pages of 1000 following the driver's paging state, up to the row limit. [[key:cancel]] stops asking for pages.

## Results
Each row is a document: **Documents** shows the primary key and the first fields on the left, the row as a tree on the right; **Table** puts it in a grid. Lists, sets, maps and user types become JSON; decimals and large integers arrive as text so they stay exact; dates and instants as `toJson()` writes them. In the **Table**, **Copy as › INSERT** writes one `INSERT INTO … JSON '…'` per row, which Cassandra reads back with the column types.

## Editing rows
Rows of a `SELECT *` on a table can be edited, found by primary key:
- **Edit** opens the row as JSON; **Save** writes it again with `INSERT … JSON`. The key columns do not change, even if you correct them in the text.
- **Duplicate** opens a copy: change the key before **Insert**, or NiceDB says the row already exists.
- **Delete** removes the row by key.

If someone else removed the row meanwhile, NiceDB says so instead of creating it again. In production every edit goes through the write gate and lands in the audit log as CQL ([Production safety](production)).

## Writes and production
`SELECT`, `DESCRIBE`, `USE`, `LIST` and `CONSISTENCY` read; everything else writes. `DROP`, `TRUNCATE` and `ALTER` ask for confirmation as risky.

## Explain: tracing
**Explain** runs the read with tracing on and draws it as a plan: one branch for each node that worked, with its events in order and the time of each. The slowest step is highlighted. Writes are not traced, since tracing would run them.

## Tools › Cluster
Cluster name, version, nodes with datacenter, rack and tokens, the token ring with each node's range, and the keyspaces with their replication.
