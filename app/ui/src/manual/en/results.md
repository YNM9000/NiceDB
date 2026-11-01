# Running queries and results

## Run
| Command | What it runs |
|---|---|
| **Run**, [[key:run_statement]] | the statement under the cursor; with a selection, the selection |
| **Run all**, [[key:run_script]] | the whole text; with a selection, the selection |
| right-click › **Run selection** | only the selected text |
| **Explain** | the plan of the statement under the cursor: [ER diagram and query plans](er-plans) |
| **Dry run** | the write inside a transaction you then confirm: [Transactions and dry run](transactions) |

If the statement holds placeholders such as `:id`, the parameters bar appears above the results: [Query parameters](parameters).

Before an `UPDATE` or `DELETE` without `WHERE`, a `DROP`, a `TRUNCATE` or an `ALTER`, NiceDB asks for confirmation and tells how many rows would be touched: **Run anyway** goes ahead, **Cancel** backs out. On production connections the rules of [Production safety](production) apply.

After a `CREATE`, `ALTER` or `DROP` the schema reloads on its own.

## Stop a query
Press **Cancel** in the query bar or [[key:cancel]]. The query is cancelled on the server, not just hidden. Rows that already arrived stay in the grid.

With ClickHouse, progress shows while the query runs: rows read, bytes and percentage.

## Result tabs
| Tab | Content |
|---|---|
| **Results** | the grid; with several statements returning rows, **Results 1**, **Results 2**… |
| **Chart** | the rows as a chart: [Value panel and charts](value) |
| **Before and after** | the rows touched by a dry run |
| **Messages** | each statement with its outcome, rows affected and time |
| **Plan** | the plan after **Explain** |

Next to the tabs: row count and time, **Value**, **Copy** and **Export** ([Import and export](import-export)). Right-clicking **Results** offers **Compare with…** ([Tools](tools)) and **Export**.

**Messages** opens on its own when a statement fails or returns no rows. An error shows the server's message, line and column, details and **Hint** when the server gives them; the error spot is also marked in the editor.

## Move and select in the grid
| Key or gesture | Effect |
|---|---|
| arrow keys | next cell |
| [[Ctrl]] + arrow | to the edge of the grid |
| [[PgUp]] / [[PgDn]] | one page |
| [[Home]] / [[End]] | start and end of the row; with [[Ctrl]], first and last cell |
| [[Shift]] + arrows or click | extends the selection |
| [[Ctrl]] + click | adds or removes cells |
| click a row number | selects the row; drag for more rows |
| [[Shift+Space]] / [[Ctrl+Space]] | selects the cell's row / column |
| [[Ctrl+A]] or click the corner | selects everything |
| [[Esc]] | keeps only the active cell |
| right edge of a header | drag to widen the column; double-click to fit the content |

[[Shift+Enter]] opens the value panel for the cell: [Value panel and charts](value). Right-clicking a cell offers copying, sorting, selection and **Compare with…**.

## Selection totals
With at least two cells selected, the status bar shows **Count**, **Sum**, **Average**, **Min**, **Max** and **Distinct**. Sums are exact even with many decimals. Clicking a value copies it.

## Copy
- [[Ctrl+C]] copies the selection as tab-separated text.
- **Copy** above the grid: **As text (TSV)**, **As CSV**, **As JSON**, **As INSERT**.
- Right-click › **Copy as**: **Text with headers**, CSV, JSON, INSERT; then **Copy column** and, on the header, **Copy name**.

## Sort
Clicking a header sorts the column: ascending, descending, then unsorted. [[Shift]] + click adds the column to the sort. The same items are in the right-click menu: **Sort ascending**, **Sort descending**, **Remove sort**.

Sorting happens on the server: NiceDB runs the statement again with an `ORDER BY` and swaps the rows in. [[Ctrl+Z]] goes back to the previous order. A result that is not from a `SELECT`, that writes, or that comes from several statements at once cannot be sorted: the reason shows above the grid.

## Long results
- **Settings › Results › Row cap per query** stops reading past a number of rows: "Showing the first N rows: narrow the query to see them all."
- The connection's **Row limit** cuts `SELECT`s without `LIMIT`: "Connection limit: showing the first N rows" appears with **Load all**, which reads the statement again without that limit and keeps the sort. See [Connections and profiles](connections).

## Dropped connection
If the connection drops during a query, a notice appears above the results. The query is never run again on its own:
- for a read, **Run again from here** runs it again safely;
- for a write the notice says it may have run, and the button is **Run again anyway**;
- if the connection does not come back, **Reconnect** tries again.

## History
**History** in the query bar, or [[key:panel_history]], opens the list of queries run on this connection, with date, time and rows. Failed queries are marked.
- **Search history** filters by text.
- A click inserts the query at the end of the editor.
- Right-click: **Insert into editor**, **Open in a new tab**, **Copy**.

## Limitations
- The results grid is read-only: to change rows use the **Data** view ([Data view and editing](data)).
- Sorting runs the query again: on a slow query the new order takes just as long.
- MySQL cannot sort a result with two columns of the same name.
- The connection limit stops reading by cancelling the query: the server may have computed more rows already.
