# Getting started

## First-time setup
On first launch, after the opening screen, the **First-time setup** appears. Every choice shows at once in the **Preview**.

| Step | What you choose |
|---|---|
| **Language** | language of menus, messages and warnings |
| **Appearance** | style, palette and light or dark theme |
| **Comfort** | interface size, editor font size, sounds and volume |
| **Production safety** | how long writes stay unlocked and how a write is confirmed |
| **Ready** | a summary of your choices |

- [[Enter]] moves to the next step, **Back** returns to the previous one.
- [[Esc]] or **Skip** closes the setup after a confirmation: anything you have not chosen stays as it is.
- To see it again: **Settings › Appearance › Review the initial setup**.

Every choice can be changed later in [Settings](settings). The safety options are explained in [Production safety](production).

## Add your first connection
1. Press **New connection** in the middle of the window, or the **+** next to **Connections** in the left column.
2. Pick the engine in the list on the left: type a few letters to find it, the arrows change it, [[Enter]] moves to the fields.
3. Fill in **Name**, then host, port, database, user and password; SQLite and DuckDB only need the file.
4. Press **Test**: the server's product, version and database appear next to the button.
5. Press **Save**. The connection shows up in the **Connections** column.

All fields, the SSH tunnel, TLS and session options are covered in [Connections and profiles](connections).

## Write and run a query
1. Click the connection in the left column: a tab opens with the editor and, below it, the results area.
2. Type a query.
3. Press [[key:run_statement]] to run the statement under the cursor, or [[key:run_script]] to run the whole text.
4. Rows arrive in the grid below the editor; errors and counts are in the **Messages** tab.

To stop a running query press [[key:cancel]] or **Cancel**. More in [Editor and completion](editor) and [Running queries and results](results).

## The window
| Area | What it holds |
|---|---|
| Title bar | on the left the logo, the workspace and the connection name, with the **production** badge when it applies; on the right the views, **Search**, **Help** and the **Settings** gear |
| **Connections** column | folders, connections and, under the open connection, the tree of database objects |
| Tab row | one tab per query; on the right the commit mode, the open transaction and the **Notes, scripts and snippets** button |
| Query bar | **Run**, **Run all**, **Explain**, **Dry run**, **Format**, **Save script**, **History** |
| Editor and results | split by a bar you can drag; with the bar focused, [[↑]] and [[↓]] move it |
| Status bar | session state, totals of the selected cells, number of connections |

Side columns get wider by dragging their edge; widths are remembered per workspace.

## Views
The switcher in the title bar changes the view of the active tab:

| View | What it is for |
|---|---|
| **Query** | editor and results |
| **Data** | the table picked in the tree, to browse and edit: [Data view and editing](data) |
| **Schema** | the table diagram: [ER diagram and query plans](er-plans) |
| **Tools** | server activity, space, users, comparisons, backups: [Tools](tools) |

The switcher follows the active tab and stays hidden until a tab is open. From the keyboard: [[Tab]] to the switcher, then the arrow keys.

## Search and run commands
**Search**, or [[key:palette]], finds tables, connections, scripts and commands. [[key:palette_commands]] opens the commands straight away. See [Command palette and shortcuts](palette).

## Open the help
[[F1]] or the **Help** button (the question mark next to the gear) opens this manual on the page for the screen you are on. With tabs open it opens beside your work, in a column on the right: you can read and write the query at the same time. The command palette has it too: “Open the manual”.

| Key | In the manual |
|---|---|
| [[/]] | search every page; [[Enter]] opens the first hit, [[↓]] moves into the hits |
| [[Alt+←]] [[Alt+→]] | previous or next page |
| [[↑]] [[↓]] | in the contents, move from page to page |
| [[Esc]] (with the focus in the manual) or [[F1]] | closes the manual |

Animations play on their own; the button at the top right pauses them. With reduced motion turned on in the system you see the first frame, and the animation plays only from the button.

## Undo
[[Ctrl+Z]] undoes the last action: a closed tab, a deleted connection, a moved folder, a sort. Many actions show a notice with **Undo**. [[Ctrl+Shift+Z]] or [[Ctrl+Y]] redo. Commit and Rollback cannot be undone: see [Transactions and dry run](transactions).
