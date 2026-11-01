# Table designer

The table designer changes a table's structure, or creates a new table, without writing `ALTER TABLE` by hand. As you edit, the exact script that will run appears on the right.

## Open the designer
- Right-click a table, in the explorer or on a box of the ER diagram › **Edit table…**
- Right-click a schema or the **Tables** group › **New table…**: the same tab opens, empty.
- From the command palette ([[key:palette_commands]]): "Edit the table structure" with a table open in the **Data** view, or "New table…".

The tab is named "customers · structure" and starts from the table as it is on the server.

## The four parts
| Part | What you change |
|---|---|
| **Columns** | **Name**, **Type**, **Length**, **NULL**, **Default**, **Auto** (automatic numbering), **Comment**; the key beside the name adds the column to the primary key or takes it out; the arrows change the order |
| **Indexes** | name, **Columns, in order**, **Unique** |
| **Foreign keys** | name, **Referenced table**, column pairs (**Another column** adds one), **On delete**, **On update** |
| **Checks** | name and **Condition**, for example `price >= 0` |

- **Type** suggests the engine's types (`varchar`, `numeric`, `nvarchar`, `INT8`…) but accepts any type.
- **Length** is the part in brackets: `80` for `varchar(80)`, `10,2` for `numeric(10,2)`, `max` for `nvarchar(max)`.
- **Default** is an SQL expression as the engine expects it: `0`, `'Brescia'`, `now()`, `CURRENT_TIMESTAMP`.
- A green mark on the left means a new column, a yellow one a changed column.
- Renaming a column updates the primary key, indexes and foreign keys that use it.
- **Remove** also takes a column out of keys and indexes; the column moves to **To be removed**, where **Restore** brings it back.
- **Discard changes** returns everything to the table as the server has it.

## The script
On the right, **Script** rewrites itself at every change: `CREATE TABLE` for a new table, the `ALTER TABLE` statements in a safe order for an existing one. Notes above the script say what the change implies:

| When | Note |
|---|---|
| a column is removed | "Column note is dropped with its data." |
| SQLite, libSQL or DuckDB cannot make the change in place | "… cannot change this in place: the table is created anew and its rows copied." |
| MySQL or MariaDB move a column | "Moving note rewrites the whole table: slow on large tables." |
| PostgreSQL, CockroachDB or SQL Server and a new column order | "… cannot reorder columns: the server's order stays." |
| a new NOT NULL column without a default | "… it fails if the table already has rows." |
| a type change | "The values of price are converted: if one does not fit the new type, the script stops." |
| the engine has no comments or cascading actions | the note says what is left out |

While something is missing (the table name, a column type, a check condition, two columns with the same name) the script stays empty and the note says what to fix: "Fill in the flagged fields to see the script."

**Copy script** puts it on the clipboard, **Open in editor** writes it into a new query tab without running it.

## Apply
1. Review the script and the notes.
2. Press **Apply** (for a new table, **Create table**). The script goes through the editor's checks: risk analysis, unlock and confirmation on production, write log and history.
3. On PostgreSQL, SQL Server, SQLite, libSQL and DuckDB the script runs **as a trial**, inside a transaction: "Script run as a trial. Nothing is final until you confirm."
4. Check the result, then **Confirm** to make it final or **Roll back** to leave the table as it was. On production, confirming asks for the connection name.
5. After confirming, the schema reloads and the tab starts again from the updated table.

![Changing a column in the table designer and applying the script](anim:designer-apply)

On MySQL, MariaDB and CockroachDB each statement is final as soon as it runs: the script starts at once and, if a statement fails, the earlier ones stay. When a statement fails, the error shows above the script and the trial is rolled back.

## How the table changes, by engine
| Engine | How |
|---|---|
| PostgreSQL | `ALTER COLUMN … TYPE … USING`, `SET/DROP NOT NULL`, `SET/DROP DEFAULT`, identity, `COMMENT ON` |
| CockroachDB | as PostgreSQL; the primary key changes with `ALTER PRIMARY KEY` and cannot be dropped |
| MySQL, MariaDB | `RENAME COLUMN`, `MODIFY COLUMN` with the full definition, `AFTER` or `FIRST` for the order |
| SQL Server | `sp_rename`, `ALTER COLUMN`, defaults as `DF_table_column` constraints, comments with `sp_addextendedproperty` |
| SQLite, libSQL | adding, renaming and dropping columns and indexes in place; anything else rebuilds the table and copies its rows |
| DuckDB | type, NULL, defaults, renames and comments in place on a table without indexes; anything else rebuilds the table |

## Triggers and rebuilds
When SQLite or libSQL have to rebuild the table, the script puts the triggers back after the indexes. If a trigger uses a column the change drops or renames, or the table's old name, the script stays empty, **Apply** is disabled and the note names the trigger: "Trigger order_lines_price_au uses price, which this change takes away: the table cannot be rebuilt." Change or drop the trigger, then try again.

## Limitations
- Views that read the table are not touched; renaming the table does not update references in views and functions.
- On PostgreSQL a `serial` column shows its `nextval(…)` default and no **Auto** tick.
- Two columns swapping names in the same change cause a server error: do it in two steps.
- Partial indexes, expression indexes, indexes with included columns or descending order show up as plain indexes: editing them loses those parts.
- Unnamed SQLite checks get a stand-in name (`table_check_1`).
- CockroachDB changes a column's type only in the cases it considers safe.
- On DuckDB a rebuild fails when another table has a foreign key to this one.
