# ER diagram and query plans

The **ER diagram** draws the tables of a database and the foreign keys that tie them. The **query plan** shows how the server runs a query, where the time goes and what can be improved.

## Open the ER diagram
- In the view switcher, **Schema**: the diagram of the tab's database.
- Right-click a table in the explorer › **Show in the ER diagram**: the diagram opens centered on that table.

Tables arrange themselves on first open; the layout is saved for the connection and database. The top line shows the count, for example "48 tables · 61 relations". Tables tied to each other form colored **areas**; those without relations sit in an area of their own, "unrelated".

## Move around the diagram
| Action | Effect |
|---|---|
| drag the background, or hold [[Space]] and drag | pans the view |
| mouse wheel | zooms |
| [[Shift]] and wheel | scrolls sideways |
| drag a table | moves it; the position is saved |
| click a table | opens the details panel |
| double-click a table | opens its data in the **Data** view |
| [[+]], [[-]], [[0]] | zoom in, zoom out, back to 100% |
| [[F]] | fits the diagram to the window |
| arrows | pan the view |
| [[Esc]] | clears the selection |

The buttons at the top do the same: **Zoom in**, **Zoom out**, **Back to 100%**, **Fit**. **Rearrange** computes the layout from scratch. The **Overview** at the bottom shows the whole diagram; drag inside it to move the view.

## Find and filter
- **Find table or column**: type, choose with [[↑]] [[↓]], and [[Enter]] takes the view to the table.
- **Filter** opens checkboxes for the schemas and, inside each schema, for the areas: unticked tables disappear from the drawing. **Show all** brings them back. While the filter is on, the button says how many tables remain.

## The details panel
Click a table: the right side shows estimated rows, **Columns** (with primary key, foreign keys, `unique`, `not null`), **Indexes**, **References** (the tables it points to) and **Referenced by**. Foreign keys without an index are marked "no index". **Open data** opens the table; **Close** closes the panel. With no table selected, the panel sums up the database: tables, relations, schemas and areas.

The **Legend** explains the relation marks: one, many, one and only one, zero or one.

## Right-click menu
On a table: **Open data**, **New SELECT query**, **Frame the linked tables**, **Show DDL**, **Edit table…**, **Generate SQL**, **Dependencies…**, **Copy name**, **Copy CREATE** (see [DDL and SQL generators](ddl) and [Table designer](designer)). On the background: **Fit** and **Rearrange**.

## Export the diagram
**Export** saves the diagram as an **SVG drawing** or a **PNG image**, in the colors of the current style, leaving out the tables hidden by the filter.

## Read a plan
1. In the editor, put the cursor on the query.
2. Press **Explain** in the editor bar (or the "Explain" command in the palette, [[key:palette_commands]]).
3. The **Plan** tab opens next to the results. If the query has parameters, the parameters bar comes first.

By default NiceDB asks for the **measured** plan: the query really runs and the plan reports actual rows and times. The header says which one you have: "EXPLAIN ANALYZE · run at 10:42" or "EXPLAIN · estimate at 10:42, nothing run".
- A writing statement is measured inside a transaction that is rolled back right after, and the plan says so.
- On production a writing statement is not run: you get the estimate only.
- On MySQL and MariaDB writes get the estimate only; so does DuckDB with a transaction already open.
- SQLite and libSQL do not measure time: the plan shows the order of the reads.

[[key:cancel]] stops a measured plan that takes too long, as it does a query.

## The Plan tab
| Control | Effect |
|---|---|
| **Drawing** | the plan as a flow from left to right: width is the number of rows passing through, the occupied track is where the time goes |
| **Slip** | the list of nodes with rows, estimates, time or cost and share |
| **Text** | the plan as the server returns it |
| **Compare** | sets the tab's previous plan alongside: "Before and after", with "3 times faster" or "slower" |
| **Estimate only** | asks for the plan without running the query |
| **Explain again** | takes the measured plan again |

Above the drawing, **What happens** sums the plan up in a sentence ("The query took 1.2 s. The heaviest step is…"), and **Where the time goes** (or **Where the cost goes**, for an estimate) splits the total across the nodes.

## Notes
Below the plan, **Notes** lists the problems found, each with a severity (**Serious**, **Worth a look**, **Minor**), an explanation and a **Suggestion** to take with **Copy**:
- a full read of a large table with a selective filter;
- a foreign key without an index;
- estimates far from reality, which call for fresh statistics;
- sorts or hashes that spill to disk;
- nested loops with many rounds;
- functions or type conversions on an indexed column, which switch the index off;
- `SELECT *` feeding a sort, huge results without `LIMIT`.

Fixes are written in the engine's dialect. Try them on a non-production environment first.

## Engines
| Engine | Measured plan | Estimate only |
|---|---|---|
| PostgreSQL | `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` | `EXPLAIN (FORMAT JSON)` |
| CockroachDB | `EXPLAIN ANALYZE (VERBOSE)` | `EXPLAIN (VERBOSE)` |
| MySQL | `EXPLAIN ANALYZE`, reads only | `EXPLAIN FORMAT=JSON` |
| MariaDB | `ANALYZE FORMAT=JSON`, reads only | `EXPLAIN FORMAT=JSON` |
| SQL Server | `SET STATISTICS XML ON` | `SET SHOWPLAN_XML ON` |
| DuckDB | `EXPLAIN (ANALYZE, FORMAT JSON)` | `EXPLAIN (FORMAT JSON)` |
| SQLite, libSQL | none | `EXPLAIN QUERY PLAN` |
| ClickHouse | none | `EXPLAIN json = 1` (see [ClickHouse](clickhouse)) |

MongoDB, Redis and Valkey have no **Explain**. The ER diagram exists for the eight relational engines; ClickHouse, MongoDB, Redis and Valkey do not have it.

## Limitations
- On SQL Server the measured plan runs the whole query on the server, even though NiceDB discards its rows: for a long query, use **Estimate only**.
- CockroachDB gives no JSON plan: a version that changes the plan's wording can leave nodes without a type, still visible in **Text**.
- DuckDB estimates rows but not costs: in an estimate the split is empty.
- Notes read the catalog for at most 3 seconds: on a very busy server, those that need indexes and foreign keys may stay silent.
- With thousands of tables the ER drawing takes a moment to appear; past a few thousand, filter by schema.
