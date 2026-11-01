# Explorer

The explorer, in the left sidebar, shows each open connection's databases and their objects in a single tree. From here you open data, start your first queries and reach the tools with a right-click.

## The tree
A relational connection expands like this:

- **Databases** › database › **Schemas** › schema › **Tables**, **Views**, **Functions**, **Sequences**, **Types**, **Triggers**
- **Users and roles** and **Active sessions**, at server level

On MySQL, MariaDB, ClickHouse and CockroachDB, databases not opened yet show how many tables they have, read from the server's catalog with a single query.

On PostgreSQL and SQL Server a database with 2000 tables and views or more, spread over several schemas, opens level by level: the list of schemas comes at once, with how many tables and views each holds, plus the objects of the first schema; the others are read when you expand them. Until a schema is opened, completion, the palette and the ER diagram do not see its tables.

Only the levels the engine really has appear: SQLite, libSQL and DuckDB are a file and go straight to schemas or tables, MySQL and MariaDB have no schemas inside a database. ClickHouse adds **Materialized views** and **Dictionaries**, MongoDB shows **Collections** and **Fields**, Redis and Valkey the key **Prefixes** (see [MongoDB](mongodb), [Redis and Valkey](redis), [ClickHouse](clickhouse)).

- Next to a table is its estimated row count (`~12,400`).
- The database of the active tab carries a dot.
- Expanding a table shows its columns and indexes.
- With thousands of tables the tree scrolls without slowing down: it draws only the visible rows.

## Open objects
| Action | Effect |
|---|---|
| Click a table or view | opens its data in the **Data** view (see [Data view and editing](data)) |
| Double-click a table or view | opens a new tab with the table's `SELECT` |
| Click a database | opens a tab on that database |
| Click the triangle | expands or collapses the node |

From the keyboard, with focus in the tree:

| Key | Effect |
|---|---|
| [[↑]] [[↓]], [[Home]], [[End]] | move the selection |
| [[→]] | expand the node, or step into its first child |
| [[←]] | collapse the node, or step up to its parent |
| [[Enter]] | same as a click |
| [[Space]] | expand or collapse |
| letters | jump to the first node starting with that text |

## Filter
- **Filter objects**, above the tree, narrows the objects as you type. [[Enter]] in the field opens the palette on **Objects** with the same text, to search every open database (see [Command palette and shortcuts](palette)).
- **Search connections**, higher up, filters the connections; [[Enter]] opens the palette on **Library**.

## Switch database and hide the ones you do not need
The bar above the filter shows the active tab's database.
- Click the name (**Switch database**): the list of the server's databases; "in use" marks those open in other tabs.
- The settings icon (**Visible databases and schemas**) opens checkboxes for the databases and schemas to show, with **Search**, **Only those in use** (keeps just the ones your tabs use) and **Defaults**.
- Right-click a database › **Hide the database**, a schema › **Hide the schema**. [[Ctrl+Z]] brings them back.
- With hidden objects, **Show all** appears under the bar with a count ("3 hidden"); **Hide again** returns to your choice.
- **Refresh schema** (the circular arrow) reads the schema again from the server, for example after a `CREATE TABLE` run from another program.

## Right-click menus
Each node has its own menu. Tool entries appear only when the engine has the tool (see [Tools](tools)).

| Node | Main entries |
|---|---|
| Table | **Open data**, **New SELECT query**, **Show in the ER diagram**, **Show DDL**, **Edit table…**, **Generate SQL**, **Dependencies…**, **Copy name**, **Copy SELECT**, **Copy CREATE**, **Export data…**, **Import data…**, **Test data…**, the engine's maintenance, **Show columns** |
| View | as for a table, without editing, import, test data and maintenance |
| Database | **Open a tab on this database**, **New query here**, **Find a value…**, **Schema compare**, **Backup and restore**, **Security**, **Disk usage**, **Monitor**, the database's maintenance, **Copy name**, **Hide the database** |
| Schema | **New query here**, **New table…**, **Find a value…**, **Copy name**, **Hide the schema** |
| Tables group | **New table…** |
| Function, procedure | **Show DDL**, **Copy CREATE**, **Copy signature**, **Copy name** |
| Trigger, sequence, type, index | **Show DDL**, **Copy CREATE**, **Copy name** |
| Column | **Copy name**, **Copy qualified name** |
| User or role | **Permissions…** (also by clicking the node: it opens on that user), **Copy name** |
| Active session | **Cancel query**, **Close session**, **Copy query**, **Open server activity**, **Monitor** |

Almost every menu ends with **Refresh schema**. DDL, generators and dependencies are covered in [DDL and SQL generators](ddl), structure editing in [Table designer](designer).

## Maintenance
On tables and databases the menu offers the maintenance the engine has; each entry shows on the right the statement it will use.

| Engine | Table | Database |
|---|---|---|
| PostgreSQL | **Clean up** (`VACUUM (ANALYZE)`), **Update statistics** (`ANALYZE`) | the same on the whole database |
| MySQL, MariaDB | **Rebuild the table** (`OPTIMIZE TABLE`), **Update statistics** (`ANALYZE TABLE`) | none |
| SQL Server | **Update statistics** (`UPDATE STATISTICS`) | `sp_updatestats` |
| CockroachDB | **Update statistics** (`ANALYZE`) | none |
| SQLite, libSQL | **Update statistics** (`ANALYZE`) | **Clean up** (`VACUUM`), **Update statistics** |
| DuckDB | **Update statistics** (`ANALYZE`) | **Update statistics**, **Clean up** (`CHECKPOINT`) |

1. Pick the entry.
2. Confirm the statement. On production, writes must be unlocked and confirmed as for any write (see [Production safety](production)).
3. The work runs on a separate connection, so the editor stays free; a notice says how long it took when it ends. The operation is recorded in the write log.

## Limitations
- The explorer works on the schema read when the connection opened: objects created by other programs show up after **Refresh schema**.
- Each expanded database keeps a session open while its node stays open.
