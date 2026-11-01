# Data view and editing

The **Data** view shows a table in pages of 500 rows and lets you edit it like a spreadsheet. Changes stay pending until you press **Apply**, which writes them all together in one transaction.

## Open a table
- Click a table in the explorer, or right-click › **Open data**.
- In the view switcher, pick **Data**, then the **Pick a table** list in the header: **Filter tables** narrows it down.
- From the palette: [[key:palette]], type the table name, [[Enter]].

The header shows the row count (with a tilde when it is an estimate, such as "~3,000,000 rows"), the schema and the key used to tell rows apart. The arrows at the bottom right change page; **Reload** reads the page again from the server.

## Filter and sort
- **WHERE condition**: type the condition that follows `WHERE`, for example `credit_limit > 1000 and state = 'OH'`, and confirm with [[Enter]].
- **Filter** opens a small form: **Column**, **Condition** (`=`, `≠`, `contains`, `<`, `≤`, `>`, `≥`, `is NULL`, `is not NULL`), **Value**, then **Add**. Each filter becomes a chip next to the WHERE field; its × removes it.
- Right-click a cell › **Filter by this value** adds a chip with that value.
- Click a column header to sort on the server; hold [[Shift]] to add the column to the current sort. The header menu has **Sort ascending**, **Sort descending** and **Remove sort**.

Filters and sorting are undone with [[Ctrl+Z]] like any other action.

## Edit rows
1. Select a cell and start typing, or press [[F2]], [[Enter]] or double-click.
2. Confirm with [[Enter]] (moves down a row) or [[Tab]] (moves to the next column); [[Esc]] leaves the value as it was.
3. Repeat on other cells. Each changed cell stays highlighted and the bottom bar counts the changes: "3 cells changed · 1 new row".
4. Press **Apply** (or [[Ctrl+S]]).

![Editing a cell, reviewing the pending changes and applying them](anim:data-edit)

| Key | Effect |
|---|---|
| [[F2]], [[Enter]], double-click | edit the cell |
| [[Backspace]] | edit the cell starting from empty |
| [[Del]] | set the cell to NULL |
| [[Ctrl+Del]] | mark the row for deletion, or restore it |
| [[Ctrl+Z]] | undo the last pending change |
| [[Ctrl+Shift+Z]], [[Ctrl+Y]] | redo the undone change |
| [[Ctrl+S]] | apply |

- **New row** adds an empty row at the top, marked "new". Columns left empty get the server's default value.
- Right-click a cell or a row number: **Edit cell**, **Set NULL**, **Delete row** (or **Delete rows** with several rows selected), **Restore row**, **Drop new row**.
- Binary columns are typed as hex, `\x0a1f` or `0x0A1F`.
- Foreign key columns open a list of the linked rows while you type: see [Foreign key navigation](fk).
- Long values, JSON and XML are easier to edit in the [Value panel](value).

## Review before applying
The bottom bar has **Show SQL**: it lists the statements **Apply** will run, one per change, under the note "SQL to be run · single transaction". Next to each statement, **Discard this change** removes only that one; **Copy SQL** copies them all. **Discard all** drops every pending change.

## Apply
- All changes run in a single transaction: if one fails, nothing is written and the failing row stays marked with the server's message.
- Before updating or deleting, NiceDB checks that the row on the server is still the one it read. If someone changed it in the meantime, "Changed by someone else after it was read" appears and nothing is applied: reload and try again.
- When the tab does not commit by itself (manual or smart commit, or an open transaction), the changes go into the transaction and must be confirmed with **Commit**: see [Transactions and dry run](transactions).
- On a production connection the view stays read-only until writes are unlocked, and **Apply** asks for confirmation: see [Production safety](production).
- After a successful **Apply** there is nothing left to undo in the view: the changes are on the server.

## When editing is not possible
A padlock with the reason takes the place of **New row**:
- "Read-only: this is a view."
- "Read-only: the table has no primary or unique key." Without a key NiceDB cannot tell which row to change.
- "Production is read-only: unlock writes to edit."
- On ClickHouse rows are not changed one at a time: use `INSERT`, `ALTER TABLE … UPDATE` or `ALTER TABLE … DELETE` in the editor.

## Write log
**Log**, in the header, opens the connection's write log on the right: every applied change with time, user, database and rows touched, together with writes made from the editor and the tools. **Export** saves it as a **CSV table** or a **JSON file**. The log takes turns with the **Links** panel and the value panel: only one shows at a time.

## Copy
- [[Ctrl+C]] copies the selected cells as tab-separated text, ready to paste into a spreadsheet.
- Right-click › **Copy as**: **Text with headers**, **As CSV**, **As JSON**, **As INSERT**.
- On a header: **Copy name**, **Copy column**, **Fit width** (also a double-click on the column edge).

## Limitations
- Editing works on the loaded page: to change thousands of rows, an `UPDATE` from the editor is the better tool.
- The view does not edit views, even where the engine would allow it.
- Chart, value panel and aggregates work on the current page, not on the whole table.
