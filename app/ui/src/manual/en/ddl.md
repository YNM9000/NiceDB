# DDL and SQL generators

Right-click an object, in the explorer or on a box of the ER diagram, to read its definition straight from the server, write the most common statements for a table, and see what depends on what.

## Read the DDL
1. Right-click a table, view, function, procedure, trigger, index or sequence.
2. **Show DDL**.
3. A window opens with the highlighted create statement, read-only: "Written by PostgreSQL, read-only."
4. **Copy** puts it on the clipboard, **Open in editor** writes it into a new query tab, **Close** closes the window.

**Copy CREATE**, in the same menu, copies the DDL without opening the window. **Show DDL** is in the palette too: [[key:palette]], search for the object, [[→]] for its actions.

The DDL is the one the engine writes, with defaults, constraints, identities and comments:

| Engine | Where it comes from |
|---|---|
| PostgreSQL, CockroachDB | rebuilt from the catalog, `pg_get_viewdef`, `pg_get_functiondef`, `pg_get_triggerdef`, `pg_get_indexdef` |
| MySQL, MariaDB | `SHOW CREATE TABLE`, `VIEW`, `FUNCTION`, `PROCEDURE`, `TRIGGER` |
| SQL Server | `sys.sql_modules` and table reconstruction |
| SQLite, libSQL | the text stored in `sqlite_schema` |
| DuckDB | `duckdb_tables()`, `duckdb_views()`, `duckdb_functions()` |

## Generate SQL
Right-click a table or a view › **Generate SQL**. The chosen entry writes the statement into a **new query tab** on the same database, without running it. **Copy to clipboard**, at the bottom of the submenu, has the same entries and copies the statement instead.

| Entry | What it writes |
|---|---|
| SELECT | every column by name, one per line |
| INSERT | the columns with a placeholder for each, except those the server numbers itself (identity, `serial`, `AUTO_INCREMENT`) |
| **UPDATE by key** | the non-key columns in `SET`, the key in `WHERE` |
| **DELETE by key** | `WHERE` on the key |
| INSERT … ON CONFLICT, INSERT … ON DUPLICATE KEY, MERGE | the upsert: inserts the row or, when the key already exists, updates it; the entry is named after the engine's syntax |
| **Copy of the table** | a `<name>_copy` table with the same columns and rows |
| DROP TABLE, DROP VIEW | the drop of the object |

```sql
UPDATE invoices
SET
  data = :data,
  total = :total,
  paid = :paid
WHERE number = :number;
```

- Values are `:column` placeholders: the first run shows the parameters bar (see [Query parameters](parameters)).
- The key is the primary key; without one, a unique index on NOT NULL columns. The hint next to the entry says which ("by number"). Without either, UPDATE, DELETE and upsert stay disabled with "no primary key".
- Names get the engine's quotes only when needed (`"Name"`, `` `order` ``, `[user]`); the schema appears only when it is not the default one.
- On views there are only SELECT, **Table from the view** and DROP VIEW.
- A DROP opens in the editor like the other entries; when you run it, the risk check asks for confirmation and on production writes must be unlocked.

Upsert and copy by engine:

| Engine | Upsert | Copy of the table |
|---|---|---|
| PostgreSQL, CockroachDB | `ON CONFLICT (key) DO UPDATE SET c = EXCLUDED.c` | `CREATE TABLE … (LIKE … INCLUDING ALL)` plus `INSERT … SELECT` |
| MySQL, MariaDB | `ON DUPLICATE KEY UPDATE c = VALUES(c)` | `CREATE TABLE … LIKE …` plus `INSERT … SELECT` |
| SQL Server | `MERGE … USING (VALUES …) AS s`, between `SET IDENTITY_INSERT … ON` and `OFF` when the table has an identity | `SELECT * INTO … FROM …` plus the primary key |
| SQLite, libSQL, DuckDB | `ON CONFLICT (key) DO UPDATE SET c = EXCLUDED.c` | `CREATE TABLE …` with columns, types, `NOT NULL` and primary key, plus `INSERT … SELECT` |

With the table open in the **Data** view, the command palette ([[key:palette_commands]]) has "Generate the table's SELECT", "Generate the table's INSERT", "Generate the UPDATE by key", "Generate the DELETE by key", "Generate the table's upsert" and "Show the table's dependencies".

## See dependencies
1. Right-click a table or a view › **Dependencies…**
2. The window splits the tied objects into two groups: **Uses** (for a view, **Reads from**) and **Used by**.
3. Click a table or view in the list to see its own dependencies; **Back** returns to the previous one.
4. On the right of each row, **Open data** and **Show DDL**.

| Link | Meaning |
|---|---|
| foreign key `name` | the table points to the other one, or the other one points here |
| in the definition | a view, function or trigger names the object in its text |
| trigger · `function` | a trigger on the table and the function it runs |
| the table's sequence | a sequence that fills a column (`serial`, identity) |

On PostgreSQL, CockroachDB and SQL Server dependencies come from the server's catalog. On MySQL, SQLite and DuckDB the engine itself reads the views; routines, macros and trigger bodies (and views on MariaDB and remote libSQL) are searched in the text of their definitions, and the window says so at the bottom.

## Limitations
- INSERT also lists columns with a default or an identity: on SQL Server an `IDENTITY` column has to be removed by hand.
- **Copy of the table** copies structure and rows; on SQLite, DuckDB, SQL Server and from views it carries no keys, indexes or constraints. It does not check whether `<name>_copy` already exists.
- On PostgreSQL a function shows up among the dependencies of a view that uses it, not among those of the tables it reads.
- The text search also counts a name quoted in a comment or a string, and does not tell apart two tables with the same name in different schemas.
- Dependencies cover tables and views; for functions, triggers and sequences use **Show DDL**.
