# Editor and completion

The editor speaks the dialect of the tab's connection. For MongoDB and Redis it takes the engine's own commands: see [MongoDB](mongodb) and [Redis and Valkey](redis).

## Editor keys
| Key | Effect |
|---|---|
| [[key:run_statement]] | runs the statement under the cursor, or the selection |
| [[key:run_script]] | runs the whole text, or the selection |
| [[key:cancel]] | stops the running query |
| [[key:format]] | formats the selection, or the whole text |
| [[Ctrl+/]] | comments or uncomments lines |
| [[Ctrl+Space]] | opens completion |
| [[Ctrl+F]] | find and replace; [[F3]] and [[Shift+F3]] go to the next and previous match |
| [[Ctrl+Alt+G]] | go to line |
| [[Alt+↑]] / [[Alt+↓]] | moves the line up or down |
| [[Tab]] / [[Shift+Tab]] | indents and outdents |
| [[key:rename_alias]] | renames the alias under the cursor |
| [[Ctrl+Z]] / [[Ctrl+Shift+Z]] | undo and redo in the text |

Shortcuts named after an action can be changed in **Settings › Keyboard shortcuts**. Right-clicking in the editor offers the same actions, plus **Run the script past errors** and **Dry run**.

## Completion
It opens on its own while you type a name, after a dot, after `JOIN` and after `ON`; [[Ctrl+Space]] opens it anywhere. [[Enter]] accepts, [[Esc]] closes. Suggestions come from the schema already loaded: no query goes to the server while you type.

| Where | What it suggests |
|---|---|
| after `FROM`, `JOIN`, `INTO`, `UPDATE`, `DELETE FROM` | tables, views, `WITH` tables, schemas |
| after `JOIN` | complete joins from foreign keys first |
| after `ON` | the join condition from foreign keys |
| after `alias.` or `table.` | that table's columns, with their type |
| after `schema.` | the schema's tables, views and functions |
| in `SELECT`, `WHERE`, `GROUP BY`, `ORDER BY`, `SET` | columns of the query's tables, aliases, functions, keywords |
| inside `INSERT INTO t (…)` | the columns of `t` |
| after `::` and in `CAST(… AS` | the engine's types |
| after `USE` | the server's databases |
| at the start of a statement | `SELECT`, `INSERT INTO`… and snippets |

```sql
select * from customers c join |        -- invoices i on i.customer_id = c.id
select r.| from (select id as customer, city from customers) r   -- customer, city
```

- Aliases of tables, subqueries and `WITH` are understood; a correlated subquery also sees the outer tables.
- A column present in two of the query's tables comes qualified (`o.id`, `c.id`).
- Tables in the default schema go in by name alone, others with the schema in front. On PostgreSQL and CockroachDB the default schema is the session's `search_path`: it is read when the session opens and again after a `SET search_path` run in the tab; a table off the path goes in with its schema.
- Quotes are added only when needed, in the dialect's style: `"Web Orders"`, `` `Web Orders` ``, `[Web Orders]`.
- A function goes in with its parentheses and the cursor between them.
- Keywords follow the case of the statement.
- Frequent and recent choices rise to the top.

A mark on the left of each suggestion tells its kind: **T** table, **V** view, **W** `WITH` table, **C** column, **ƒ** function, **⋈** join from a foreign key, **S** schema, **D** database, **A** alias, **τ** type, **§** snippet.

## Column card
When a column is picked in the list, its card shows beside it: type, nullable or not, default, comment, identity, the foreign key it points to, what points to it and the indexes that hold it. The same card opens when the mouse rests on a column in the text (also behind an alias, `f.total`) and, in the explorer, with a click or [[Enter]] on a column or from right-click › **Column info**. Default and comment are read from the server catalog the first time they are needed.

## Name warnings
A table the loaded schema does not know, or a column the table lacks, gets a dotted underline. The tooltip reads "Unknown table: …" or "… has no column …"; on PostgreSQL, for a table that exists but off the `search_path`, "… is not on the search_path: it lives in …". The warning never stops a run and stays quiet where it cannot know: DDL statements, tables created by the script itself, temporary tables, system schemas, table-valued functions.

Turn them off in **Settings › Editor › Underline unknown names**, or from the palette.

## Rename an alias
1. Put the cursor on an alias, where it is defined or where it is used.
2. Press [[key:rename_alias]], or right-click › **Rename alias**.
3. Type: every occurrence in the statement changes together.

It also works on `WITH` names, not on real tables and columns.

## Multiple cursors
| Key | Effect |
|---|---|
| [[Ctrl+D]] | selects the next occurrence |
| [[Ctrl+Shift+L]] | selects every occurrence |
| [[Ctrl+Alt+↑]] / [[Ctrl+Alt+↓]] | adds a cursor above or below |
| [[Ctrl]] + click | adds a cursor |
| [[Alt]] + drag | rectangular selection |
| [[Esc]] | back to a single cursor |

Run and Format work on the main cursor.

## Scripts with several statements
- Statements are separated by `;`. On SQL Server a line holding only `GO` ends the batch; `GO 5` runs it five times.
- On MySQL and MariaDB the mysql client's `DELIMITER` works: after `DELIMITER $$` statements end at `$$` and a procedure body stays whole; `DELIMITER ;` goes back to the semicolon. `DELIMITER` lines and the delimiters are coloured apart.
- A script stops at the first statement that fails. **Run the script past errors** (right-click or palette) goes to the end; to always do so: **Settings › Editor › Error in a script › Continue**.
- It stops anyway when the error has broken the open transaction (PostgreSQL, CockroachDB, DuckDB).

While running, the current statement is highlighted; after an error, the spot the server points to is marked in the text.

## Editor settings
In **Settings › Editor**: **Font size**, **Indent width**, **Autocomplete**, **Uppercase keywords when formatting**, **Underline unknown names**, **Error in a script**. See [Settings](settings).

## Limitations
- The schema is the loaded one: a table created from another client shows up after the schema is refreshed.
- A `search_path` changed by a function or by another connection shows at the next run that names it, or when the schema is reloaded.
- The columns of a table-returning function are known only if you name them in the alias, `x(a, b)`.
- `DELIMITER` must start a statement, on a line of its own, as in the mysql client.
