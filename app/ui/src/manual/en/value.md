# Value panel and charts

The **value panel** opens the whole content of a cell to the right of the grid: JSON as a tree, indented XML, long text, images, binaries. It works in query results and in the **Data** view. The same grid area also offers selection aggregates and a chart of the results.

## Open the panel
1. Select a cell in the grid.
2. Press [[Shift+Enter]], or **Value** in the results bar or in the Data view header.
3. Move through the grid with the mouse or the arrows: the panel follows the selected cell.

![Opening the value panel on a JSON cell](anim:value-panel)

- [[Alt+Enter]] opens the same cell in the **Row** tab, the whole row laid out as a list.
- From the command palette ([[key:palette_commands]]): "Show the cell value" and "Show the row as a record".
- [[Esc]] inside the panel closes it; pressing [[Shift+Enter]] again does the same.
- The panel widens from its left edge and its width is saved with the workspace.

The top line shows the column, its type, the row and what it holds (`JSON`, `XML`, `PNG`, `PDF`, **Text**, **Binary** or `NULL`), with its length or size.

## Read the value
NiceDB recognizes the content from the column type, then from the value: text that starts with `{` or `[` and is valid JSON opens as JSON even in a `text` column.

### JSON
- **Tree** shows the document as nodes; a closed object shows how many keys it has and a preview. Long arrays show a hundred items at a time, with "400 more…" at the end.
- **Text** shows the same document indented, with line numbers.
- **Search** filters by key or value and opens the branches where it finds something. [[Enter]] goes to the next match, [[Shift+Enter]] to the previous one.
- The bottom line holds the path of the chosen node, for example `$.customer.addresses[0].zip`. The icon beside it copies the path; **SQL** copies the expression that reads that value in the connection's dialect:

| Engine | Copied expression |
|---|---|
| PostgreSQL, CockroachDB | `data #>> '{customer,addresses,0,zip}'` |
| MySQL, MariaDB | `JSON_UNQUOTE(JSON_EXTRACT(data, '$.customer.addresses[0].zip'))` |
| SQL Server | `JSON_VALUE(data, '$.customer.addresses[0].zip')` |
| SQLite, libSQL, DuckDB | `json_extract(data, '$.customer.addresses[0].zip')` |

Right-click a node: **Copy path**, **Copy as SQL expression**, **Copy value**, **Open everything below**, **Close everything below**.

### Other content
- **XML**: indented one element per line, with the editor's colors. If the tags do not close properly, the text stays as it was.
- **Text**: wraps inside the panel, with line numbers. Past 2000 lines, "Show N more lines" appears.
- **Images** in PNG, JPEG, GIF, WebP, BMP, ICO and TIFF (the first page): a preview on a checkerboard, with size in bytes and pixels.
- **SVG**, in a text column or in a binary one: drawn as an image, never put into the page, so scripts and links inside the file do not run. For text SVG, **Image** and **Text** switch between the drawing and the source.
- **Other binaries**: size, type when recognized (PDF, ZIP, GZIP) and the first 256 bytes in hex.

**Copy the value** always copies the original value. **Save to file** saves it as it is, with a name such as `logo-12.png` or `data-7.json`.

## Edit a value
In the **Data** view, on an editable table:
1. Press **Edit**, or [[F2]] with the panel focused.
2. Change the text. In a `json` or `jsonb` column, **Format** indents it.
3. **Save** ([[Ctrl+Enter]]) stores it as a pending change; **Cancel** or [[Esc]] leave everything as it was; **NULL** sets the value to NULL.
4. Press **Apply** in the Data view to send it to the server (see [Data view and editing](data)).

Invalid JSON is not saved: the panel says where it breaks. In query results the button is missing; on a view, a table without a key, a row marked for deletion or production without unlocking it stays disabled, with the reason in its tooltip. 

### Binary values
In a binary column (`bytea`, `blob`, `varbinary`…) **Edit** is replaced by:
- **Load from file…**: picks a file (up to 16 MB) and puts its bytes in the cell;
- **Hex** ([[F2]]): the bytes as pairs of digits, eight per line, to fix by hand; spaces and line breaks are ignored. Over 1 MB, load a file.

Both become a pending change like the others: it shows in the bar at the bottom, [[Ctrl+Z]] undoes it and **Apply** sends it to the server.

## The Row tab
**Row** shows the row of the selected cell as a list: name and type on the left, value on the right.

| Key | Effect |
|---|---|
| [[↑]] [[↓]] | previous or next row |
| [[PgUp]] [[PgDn]] | ten rows back or forward |
| [[Ctrl+Home]], [[Ctrl+End]] | first and last row |
| [[Tab]], [[Shift+Tab]] | next or previous field |
| [[Enter]] | opens the field in the **Value** tab |
| [[Ctrl+C]] | copies the field's value |

**Copy the row** copies one line per column, `name<TAB>value`.

## Selection aggregates
With more than one cell selected, the status bar shows totals on the right, as a spreadsheet does:
- numeric cells: **Count**, **Sum**, **Average**, **Min**, **Max**;
- other cells: **Count** and **Distinct**;
- in both cases, when there are any, how many NULLs.

Count leaves NULLs out. The sum of decimals is exact (`0.1 + 0.2` gives `0.3`). Click an aggregate to copy it. With many cells the bar shows "Counting…" until the result arrives.

## Chart
The **Chart** tab, next to **Results** and **Messages**, draws the rows already read without running the query again. In the Data view, the chart button in the header shows the page as a chart in place of the grid. From the palette: "Show the chart".

NiceDB starts with a sensible chart: with a date column on X it draws lines, otherwise bars on the first text column; on Y the first numeric column that is not a key; when X repeats it sums the values and keeps the top 12 categories.

| Setting | Values |
|---|---|
| **Chart type** | **Bars**, **Horizontal bars**, **Lines**, **Areas**, **Pie**, **Scatter** |
| **X** | the category column, or the horizontal axis |
| **Y** | one or more numeric columns |
| **Aggregation** | **None**, **Sum**, **Count**, **Average**, **Minimum**, **Maximum**, grouped by X |
| **Order** | **Arrival**, **By X**, **Descending**, **Ascending** |
| **Categories** | **All** or **Top 5, 10, 20, 50**; the rest go into **other** |

- The pie uses only the first Y and only positive values. Scatter needs a numeric or date X and does not aggregate.
- Hover to read the details. From the keyboard: [[Tab]] to the chart, then the arrows, [[Home]] and [[End]].
- **Export** saves an **SVG image** or a **PNG image**, in the colors of the current style.
- Settings stay with the tab: running the query again keeps the same chart.

## Limitations
- The panel, aggregates and chart use the rows already in the grid: in results, those read up to the row limit; in the Data view, the current page. When there are more, the status bar (with whole columns selected) offers **All rows** and the chart (with an aggregation) **Group all rows**: the server does the counting, with a single read-only query.
- Image previews stop at 16 MB; a TIFF shows its first page.
- The chart draws at most 5000 categories or points.
- No dual axes and no stacked areas.
