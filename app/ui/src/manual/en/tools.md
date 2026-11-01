# Tools

The **Tools** view gathers the server-side work: sessions, load, disk usage, privileges, comparisons, searches, test data, backups, import and export, and a security check. Open it from the view switcher, by right-clicking a database in the explorer, or from the command palette ([[key:palette_commands]]), for example "Open Monitor".

The list of tools sits on the left; a tool the engine lacks stays in the list with "Not available for this engine". Every tool works on the database open in the tab.

## Which engines have them
| Tool | Engines |
|---|---|
| **Server** | Redis, Valkey |
| **Activity** | PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, ClickHouse |
| **Monitor**, **Disk usage**, **Import and export** | the eight relational engines and ClickHouse |
| **Users and privileges** | PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, ClickHouse |
| **Schema compare**, **Data compare**, **Find a value**, **Test data**, **Security** | the eight relational engines (PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL, DuckDB) and ClickHouse |
| **Backup and restore** | PostgreSQL, MySQL, MariaDB, SQL Server, SQLite, file-based libSQL, DuckDB; ClickHouse backup only |

MongoDB has no tools.

## Server
Redis and Valkey only: version, uptime, commands per second, clients, memory, keys. Below, **Info** (the `INFO` sections, with **Filter**), **Clients**, **Slow log** and **Memory**. See [Redis and Valkey](redis).

## Activity
The server's sessions with **User**, **Application**, **State**, how long, and the running **Query**. The top line sums up: "Sessions 12 · active 3 · waiting on locks 1".
- **Refresh every 3 s** reloads on its own; **Paused** stops it. **Hide idle** leaves out sessions doing nothing.
- **Cancel query** stops the query and keeps the session open; **Close session** closes it and rolls back its transaction. On production, writes must be unlocked and confirmed.
- **Who blocks whom** shows lock waits as a tree, as in Monitor.

## Monitor
One page for the state of the server, refreshed every 5 seconds (the **Automatic refresh** list goes from 2 s to a minute, or **Paused**); the choice stays after the app is closed. Refreshing stops while the page is out of sight.

| Card | What it says |
|---|---|
| **Connections** | sessions out of those allowed, active, idle in transaction, waiting for a lock |
| **Oldest transaction** | how long it has been open and who holds it; yellow past a minute, red past five |
| **Database size** | how much the open database takes |
| **Reads from cache** | the share of reads served from memory; yellow below 90 % |

- **Who blocks whom**: waiting sessions sit under the one blocking them; at the top of each branch is the session to close, with **Close session**. Under each waiting session is the lock it waits for, when the engine tells: on MySQL and MariaDB metadata locks too, such as an `ALTER TABLE` stuck behind a transaction left open ("metadata lock EXCLUSIVE on sales.orders"). Right-click on a row also offers **Explain**.
- **Open transactions** and **Databases on the server** with their sizes.
- **Slow queries**: the queries that took the most time, with calls, total and mean time. **Explain** opens the query in a tab and draws its estimated plan, without running it (see [ER diagram and query plans](er-plans)). On SQL Server the Query Store parameter declaration (`(@P1 int)SELECT …`) goes away and the `@P1` stay; on CockroachDB each `_` becomes `$1`, `$2`…: the parameters bar asks for the values, then press Explain.

Where a figure is missing, the reason takes its place: "This user cannot read it: it needs PROCESS", or how to turn on `pg_stat_statements`, `performance_schema` or Query Store. For SQLite, libSQL and DuckDB the page shows the size only. On ClickHouse, merges in progress take the place of transactions.

## Disk usage
For each table **Rows**, **Data**, **Indexes**, **Total** and **Share**. Below, **Indexes** with size and scan count: **Only never used** keeps those never read since statistics were last reset, **Copy the DROPs** and **Open in editor** prepare the statements to remove them. A never-used index may serve a monthly report: check before dropping it. On ClickHouse the **Partitions** appear too.

## Users and privileges
Roles and users on the left, with **Filter**; the chosen one on the right: attributes (**Can log in**, **Superuser**, **Creates databases**, **Creates roles**), **Member of** and **Privileges on objects**.
1. Choose **New user…**, **New role…**, **Edit…**, **Drop…**, **Grant…** or **Revoke…**.
2. Fill in the window: NiceDB writes the SQL into **SQL to run**, which you can edit.
3. **Run** sends it through the editor's checks; **Open in editor** moves it into a tab.

Passwords never go through generated SQL: set them from the editor with `ALTER ROLE` or `ALTER USER`.

## Schema compare
Compares two schemas and writes the script that brings the target in line with the source.
1. Pick a connection and schema for **Source** (as it should be, for example staging) and **Target** (to bring in line).
2. **Compare**.
3. The list shows the objects **Added**, **Removed**, **Changed**; **Show unchanged too** adds the rest. The notes "drops data" and "needs a rebuild" flag the delicate steps.
4. **Migration script**: **Copy**, **Save .sql** or **Open in editor**. The script never runs by itself; with a production target it runs only from the editor, after unlocking.

## Data compare
Compares the rows of two queries or tables by key, even across connections and engines. It also opens from a right-click on a result › **Compare with…**.
1. For **Source** and **Target**, choose **Query** or **Table**, the connection and the text or table.
2. Choose the **Key**, the columns that identify a row on both sides. NiceDB suggests the primary key.
3. **Compare**. The result counts rows **Added**, **Removed**, **Changed** and **Same**, with the old value struck through next to the new one.
4. **Difference script**: `DELETE`s first, then `UPDATE`s on the changed columns only, then `INSERT`s, in the target's dialect. Copy it, save it or open it in the editor; it never runs by itself.

Values are compared by type (`12.50` and `12.5` are equal for a number). Rows stream in read-only; for queries, an `ORDER BY` on the key helps. The comparison stops past 512 MB of rows waiting for a match.

## Find a value
Looks for a text or a number in every table of the database and says where it found it. It also opens from a right-click on a database or a schema › **Find a value…**
1. Type the **Value to find**.
2. Choose **Contains** or **Equals**, and **Match case** if needed.
3. Narrow down **Columns** (**Text**, **Numbers**, **Dates, ids and others**), **Schemas** and **Tables** (parts of the name: `ord fatt`). **Views too** adds the views, slower to read.
4. Check the estimate and press **Search** or [[Enter]].

Each table with hits shows the matching columns with their counts and the first rows. Click a column: the table opens in the **Data** view, filtered. The arrow on a row (**Open the row**) opens it in a new tab, filtered on its key. **Rows per table** and **Time limit** cap the work; **Cancel** stops the search. On production a heavy search asks for confirmation. Queries only read, but **Contains** uses no index: on large tables it reads everything. Results stay when you switch to another view and back; a search still running stops, though.

## Test data
Fills a table with made-up but believable rows, within the table's constraints. It also opens from a right-click on a table › **Test data…**
1. Choose the **Table**, the number of **Rows** (1 to 1,000,000) and the **Locale**: **Italian**, **English (UK)**, **English (US)**.
2. For each column check **Values** (the generator: name, email, amount, date, **One of…**, **From the parent table**…), its **Settings** and **NULL %**.
3. Look at the **Preview**: the first 20 rows as they will be written.
4. **Insert N rows** and confirm.

- NiceDB reads types, lengths, NOT NULL, keys, unique indexes, CHECKs, enums and foreign keys; automatic columns are left to the server. CHECKs across columns are tested on every generated row (the **Table checks** list), and unique values already in the table are not repeated.
- Name, surname, email and city in a row come from the same made-up person; emails use example domains only.
- In **One of…** a weight with `*` makes a value more frequent: `open*3, paid*6, void`.
- The **Seed** makes the rows repeatable: same seed, same rows. **New seed** picks another one.
- **Parent tables first** fills the tables this one points to before it.
- Everything runs in one transaction: at the first error, or with **Cancel**, no row is left.
- On a production connection the tool does not write, and it cannot be unlocked.

## Backup and restore
Uses the engine's own programs, with real progress.

| Engine | Backup | Restore |
|---|---|---|
| PostgreSQL | **pg_dump archive** or **SQL script**, optionally **Structure only, no data** | `pg_restore` or `psql`, in a single transaction |
| MySQL, MariaDB | **SQL script** with `mysqldump` | the `mysql` client; an error halfway leaves the work done so far |
| SQLite, file-based libSQL | **File copy**, consistent even while in use | replaces the database's content |
| DuckDB | **Folder** with `EXPORT DATABASE` | `IMPORT DATABASE`, which adds the tables |
| SQL Server | **.bak file** written by the server on its own disk | from the editor, with `RESTORE DATABASE` from the master database |

1. Choose the format, then **Save backup…** (on SQL Server, the **Path on the server** and **Back up on the server**).
2. To restore: **Choose a file and restore…** and confirm. On production, writes must be unlocked and the name typed.

If `pg_dump` or `mysqldump` cannot be found, **Choose the programs folder…** tells NiceDB where they are. CockroachDB and remote libSQL have no backup from here.

## Import and export
Exports tables and results to CSV, TSV, JSON, Excel or Parquet, and imports files into new or existing tables in a single transaction. See [Import and export](import-export).

## Security
The **Security check** reads the server's settings and tells you what is wrong, with a fix to copy. It is read-only: it changes nothing and never tries a password.
1. **Run the check**.
2. Read the findings, grouped by severity (**High**, **Medium**, **Low**) and by category: **Accounts and privileges**, **Authentication and network**, **Server version**, **Sensitive data**, **Hygiene**.
3. Each finding has **Where** and a **Proposed fix**: **Copy SQL** or **Open in the editor**. Review it before running; on production it goes through the unlock.
4. **Export PDF**, **Save Markdown** or **Copy Markdown** to hand the report over.

At the bottom, **Checks passed** and **Not checked here** (because of the engine, privileges, or data the server does not record). File engines get only a few checks: sensitive data and, for SQLite, the version.

## Limitations
- Monitor's query statistics add up since the engine's statistics were last reset; the page does not reset them.
- Data compare does not reliably write binary values back.
- Test data takes foreign key values from the first 5,000 rows of the parent table.
- The Security check's list of vulnerable versions ships with the app; **Update the list**, at the bottom of the report, downloads the signed one of the latest release. It does not replace the vendor's advisories.
