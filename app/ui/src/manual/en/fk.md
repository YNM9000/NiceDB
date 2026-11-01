# Foreign key navigation

In the **Data** view a foreign key takes you to the row it points to, a row shows what refers to it, and a trail in the header leads back. Only the foreign keys declared in the server's catalog count; relations without a constraint do not show up.

## Recognize a foreign key
- The column header carries a chain symbol; its tooltip names the target, for example "Foreign key to customers (id)".
- Values are underlined with dots. The cell under the mouse, or the selected one, shows a small arrow.
- The bottom bar adds "F4 follows the key".

## Open the linked row
1. Select a cell of the foreign key.
2. Press [[F4]], or click the arrow in the cell.
3. The target table opens in the same tab, filtered on the row: the filter is a chip (`id = 63`) next to the WHERE field.

![Following a foreign key to the linked row and back](anim:fk-navigation)

To open it in a **new tab** and leave the current one as it is: [[Shift+F4]], middle-click or [[Ctrl]]+click on the arrow, or right-click › **Open in a new tab**. The new tab is named after the row, for example `customers (id = 63)`. The menu also has **Open the linked row**.

When the key is NULL there is nothing to open and the bar says so: "Nothing to open: customer_id is NULL." With pending changes the table does not switch: apply or discard them first.

## Go back
Each step leaves a crumb to the left of the table name: `orders › customers › invoices`.
- Click a crumb to return to that point, with the filters, sort and page it had.
- [[Alt+←]], the arrow next to the crumbs or the mouse "back" button: one step back.
- [[Alt+→]] or the mouse "forward" button: redo the step you went back from.

After two steps the older crumbs fold into "…"; the full trail is in the tooltip. Picking a table from the list starts over.

The trail stays with the tab: it comes back when you reopen a closed tab with [[Ctrl+Z]], and the workspace keeps it with filters, sort and page for the next time you open NiceDB.

## Links of this row
**Links**, in the header, opens a panel on the right that follows the selected row. It appears only when the table has foreign keys, outgoing or incoming. [[F4]] on a column that is not a foreign key opens the same panel.
- **Points to**: one entry per foreign key of the table, with the column it starts from ("from customer_id") and a preview of the target row. Click to open it here, the icon on the right to open it in a new tab.
- **Pointing here**: the tables with a foreign key to this one, other schemas included, each with the number of rows that refer to the selected row. Click to open those rows, filtered.

Counts run read-only and stop after 2 seconds; those that did not arrive in time stay "—". The panel widens from its left edge and takes turns with the Log and the value panel.

## Pick a foreign key value
When you edit a foreign key cell, a list of the target table's rows opens below the cell, with the key and two descriptive columns.
1. With nothing typed, the list shows the first rows, the current one on top.
2. As you type, it searches the text in every column shown, ignoring case, and shows the first 40 matches. Each key stops the previous search; after 3 seconds without results the search stops and asks for something more precise.
3. Choose with [[↑]] [[↓]] or a click, then confirm.

| Key | Effect |
|---|---|
| [[↑]] [[↓]], [[PgUp]] [[PgDn]] | move through the list |
| [[Enter]] | confirm and move to the row below |
| [[Tab]] | confirm and stay on the row |
| [[Esc]] | cancel |

If nothing is chosen, [[Enter]] saves the text as typed, as in any other cell. The chosen value is a pending change: [[Ctrl+Z]] undoes it and **Apply** sends it to the server (see [Data view and editing](data)).

## Multi-column keys
A foreign key on several columns works everywhere: [[F4]] opens the row with one chip per column, counts compare every column, and the picker fills them all at once. When a column belongs to more than one key, [[F4]] follows the single-column one.

## Engines
PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL and DuckDB.

## Limitations
- Views have no foreign keys in the catalog: no arrows and no links.
- On a large table with no index on the key column, the "Pointing here" count may not arrive in time.
- The picker search compares text: on huge tables without suitable indexes it is a scan, stopped at the first 40 rows or after 3 seconds. The total shows only when every match fits in the list.
