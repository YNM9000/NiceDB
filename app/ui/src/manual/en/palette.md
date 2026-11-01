# Command palette and shortcuts

## Open the palette
- [[key:palette]] opens **Search everywhere** in **Objects** mode. The **Search** field in the title bar does the same.
- [[key:palette_commands]] opens it in **Commands** mode. Typing `>` at the start of a search also switches to commands.
- In the sidebar, type in the filter and press [[Enter]] to carry the search into the palette.

Both shortcuts can be changed in **Settings › Keyboard shortcuts**.

## Search
The palette has three modes; [[Tab]] moves to the next, [[Shift+Tab]] to the previous. While you type, each mode shows how many results it has.

| Mode | What it finds |
|---|---|
| **Objects** | tables, views, columns and functions of open connections, other databases, and the sessions and users the explorer has already read |
| **Library** | scripts, bookmarked queries, connections, workspaces, notes and snippets |
| **Commands** | every action in the app, with its shortcut |

- Matching is fuzzy: `rord` finds `order_lines`. An exact name, the start of a name and the start of a word count most.
- Several words must all match: one in the name, the others possibly in the context (schema, database, connection, table). `invoices number` finds the `number` column of `invoices`.
- Helper words: `table`, `view`, `column`, `function`, `session`, `kill`, `user`, `permissions`. `kill 4412` leads to session 4412.
- Objects come from the schema already loaded: no query reaches the server while you type.

## Act on a row
- [[Enter]] runs the main action, [[Shift+Enter]] the second one.
- [[→]] at the end of the text, [[Ctrl+Enter]] or right-click shows every action of the row; [[←]] or [[Esc]] goes back to the results.
- [[Ctrl+C]], with no text selected in the box, copies the row's name (for scripts, queries and snippets, the SQL).

| Row | [[Enter]] | [[Shift+Enter]] |
|---|---|---|
| Table, view | **Open data** | **New SELECT query** |
| Column | opens the table's data | **Copy name** |
| Function | **Show DDL** | **Copy signature** |
| Other database | **Open a tab on this database** | **New query here** |
| Server session | **Cancel query** | copies the query |
| Script | **Open the script** | **Run** |
| Connection | **Connect** or **Go to the connection** | **New tab** |
| Workspace | **Switch to this workspace** | |
| Command | **Run** | |

Tables and views also offer, among other actions, **Show in the ER diagram**, **Edit table…**, **Copy SELECT**, **Copy CREATE**, **Export data…**, **Import data…**, **Test data…**, the engine's maintenance and **Dependencies…**. Cancelling a query or closing a session goes through the production confirmation.

## Pinned, bookmarks and recent
- [[Ctrl+B]] pins the active row to the top, whatever it is; again, unpins it.
- [[Ctrl+D]] bookmarks a table, a view or a query.
- The “Bookmark the query” command saves the text selected in the editor, or all of it.
- With an empty box you see **Pinned**, **Bookmarks** and **Recently used** (the last eight rows chosen); while searching they rank higher.
- After closing the palette, [[Ctrl+Z]] undoes the last pin or bookmark added or removed.

## Use a two-step command
Commands ending in “…” ask for something before they run.
1. Open the commands with [[key:palette_commands]].
2. Type part of the name, for example `compare`, and pick “Compare schema with…”.
3. [[Enter]]: the box clears and the command name appears at its head; below, the list of choices.
4. Search and pick with the arrows, then [[Enter]]. When the command asks for a typed value, type it and press [[Enter]].

![A two-step command in the palette](anim:palette-two-step)

[[Esc]], or [[←]] or [[Backspace]] in an empty box, goes back to the command list.

| Command | Asks for |
|---|---|
| Compare schema with… | a connection, same engine first |
| Export *table* to… | the format: CSV, TSV, JSON, Excel, Parquet |
| Open query in… | another connection |
| Connect to…, New tab on…, Disconnect… | a connection |
| Edit / Duplicate / Delete connection… | a connection |
| Use database…, Open table…, Test data for… | a database or a table |
| Database maintenance… | an engine operation |
| Switch to workspace…, New workspace… | a workspace, or a name to type |
| Rename tab… | the new name |

## Palette keys
| Key | Effect |
|---|---|
| [[↑]] [[↓]] | previous or next row, wrapping |
| [[PgUp]] [[PgDn]] | ten rows |
| [[Ctrl+↑]] [[Ctrl+↓]], [[Ctrl+Home]] [[Ctrl+End]] | first or last row |
| [[Enter]], [[Shift+Enter]] | first or second action |
| [[→]] or [[Ctrl+Enter]], [[←]] | opens and closes the action list |
| [[Tab]], [[Shift+Tab]] | next or previous mode |
| [[Ctrl+B]], [[Ctrl+D]], [[Ctrl+C]] | pin, bookmark, copy |
| [[Esc]] | closes, or leaves the actions |

## All shortcuts
Shortcuts shown with their current binding can be changed in **Settings › Keyboard shortcuts**; the others are fixed.

### App
| Key | Effect |
|---|---|
| [[key:palette]] | Search everywhere |
| [[key:palette_commands]] | Command palette |
| [[key:new_tab]] | New tab on the active connection |
| [[key:save_script]] | Save as script |
| [[key:panel_notes]] | Notes and scripts panel |
| [[key:panel_history]] | History panel |
| [[Ctrl+Z]] | Undo (outside text fields) |
| [[Ctrl+Shift+Z]], [[Ctrl+Y]] | Redo |
| [[Ctrl+Alt+1]] | all connections |
| [[Ctrl+Alt+2]] … [[Ctrl+Alt+9]] | workspaces, in order |
| [[F1]] | Help on the current screen |
| [[F2]] | renames the focused tab, folder or script |

### Editor
| Key | Effect |
|---|---|
| [[key:run_statement]] | Run the statement under the cursor |
| [[key:run_script]] | Run the script |
| [[key:dry_run]] | Dry run |
| [[key:cancel]] | Cancel the run |
| [[key:format]] | Format |
| [[key:rename_alias]] | Rename alias |
| [[Ctrl+Space]] | opens completion |
| [[Ctrl+F]] | Find in the editor |
| [[Ctrl+D]] | selects the next occurrence |
| [[Ctrl+Shift+L]] | selects every occurrence |
| [[Ctrl+Alt+↑]] [[Ctrl+Alt+↓]] | adds a cursor above or below |
| [[Ctrl]] + click, [[Alt]] + drag | adds a cursor, rectangular selection |

### Result grid and Data view
| Key | Effect |
|---|---|
| arrows, [[PgUp]] [[PgDn]] | move the cell |
| [[Ctrl+↑]] [[Ctrl+↓]] [[Ctrl+←]] [[Ctrl+→]] | to the edge |
| [[Home]] [[End]], [[Ctrl+Home]] [[Ctrl+End]] | start or end of the row, first or last cell |
| [[Shift+Space]], [[Ctrl+Space]], [[Ctrl+A]] | select row, column, everything |
| [[Ctrl+C]] | copies the selection |
| [[Shift+Enter]], [[Alt+Enter]] | value panel, row as a record |
| [[Enter]], [[F2]] or typing | edits the cell (Data view) |
| [[Del]], [[Backspace]] | sets NULL, clears and edits |
| [[Ctrl+Del]] | marks or unmarks the row for deletion |
| [[Enter]] [[Shift+Enter]] [[Tab]] [[Shift+Tab]], [[Esc]] | while editing: confirm and move, cancel |
| [[Ctrl+S]] | Apply pending changes |
| [[F4]], [[Shift+F4]] | follows the foreign key, in a new tab |
| [[Alt+←]] [[Alt+→]] | back and forward through followed tables |

### Value panel
| Key | Effect |
|---|---|
| [[F2]] | edit |
| [[Ctrl+Enter]], [[Esc]] | save, cancel |
| [[Ctrl+Shift+C]] | copies the JSON node path |
| [[Esc]] | closes the panel |

### Explorer, sidebar and others
| Key | Where | Effect |
|---|---|---|
| arrows, [[Home]] [[End]] | explorer | move, open and close |
| [[Enter]], [[Space]] | explorer | opens, opens or closes the node |
| letters | explorer | jumps to the name starting that way |
| [[Alt+↑]] [[Alt+↓]] | connections | moves the connection or folder |
| [[Del]] | scripts | deletes the script |
| [[Enter]], [[Esc]] | parameters bar | runs, goes back to the editor |
| [[↑]] [[↓]] | Settings | previous or next section |
| [[Enter]], [[Esc]] | first-time setup | next, skip |

The key browser's keys are in [Redis and Valkey](redis).

## Limitations
- Objects come only from open connections and open databases.
- Sessions and users are read again every 5 seconds while the window is open, but only for connections where the explorer has read them once.
- Every context menu action has its command, with the same icon; those that need a connection, a format or a workspace ask for it in a second step.
- Pinned items and bookmarks do not travel in the `.nicedb` export.
- Table commands apply to the table shown in the Data view; for other tables go through **Objects**.
