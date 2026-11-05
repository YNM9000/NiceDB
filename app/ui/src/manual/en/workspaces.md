# Workspaces and folders

## Folders
Folders group connections in the **Connections** column, by client or by project, and can hold other folders.

1. Press the **New folder** icon next to **Connections**, or right-click a folder › **New subfolder**.
2. Type the name and press [[Enter]]; [[Esc]] gives up.

| Action | How |
|---|---|
| Rename | [[F2]] on the folder, or right-click › **Rename** |
| Open and close | click, or [[→]] and [[←]] |
| Give it a colour | right-click › **Colour** |
| Give it a mark (star, company, server…) | right-click › **Mark** |
| Move | drag the folder or connection; right-click › **Move to**; [[Alt+↑]] and [[Alt+↓]] change the order |
| Delete | right-click › **Delete folder**: its contents stay and move up one level |

While dragging, a closed folder opens if you hold over it; [[Esc]] cancels the move. The number next to a folder counts the connections it holds.

The **Search connections** field filters the tree as you type; [[Enter]] hands the search to **Search everywhere**.

## Workspaces
A workspace shows only some folders and connections and remembers its own tabs. **All connections** is the workspace that shows everything.

### Create a workspace
1. Open the workspace menu in the title bar and choose **New workspace…**.
2. Type the **Name**.
3. Under **Folders and connections**, tick what it should hold. A folder brings everything it holds, including what you add later.
4. Press **Create**: NiceDB switches to the new workspace at once.

From the tree: right-click a folder or a connection › **Workspaces** › **New workspace with this…**, or tick an existing workspace to add it or take it out.

### Switch workspaces
- The workspace menu in the title bar, then the workspace.
- [[Ctrl+Alt+1]] for **All connections**, [[Ctrl+Alt+2]] … [[Ctrl+Alt+9]] for the workspaces in menu order.
- From the palette: "Switch to workspace…".

When you switch, the tabs of the workspace you leave close and those of the new one come back. A restored tab connects only when you open it. If a session holds an open transaction with writes, NiceDB asks first what to do with it: see [Transactions and dry run](transactions).

### What a workspace remembers
- open tabs, with their text, title, view, table and database;
- the active tab;
- the split between editor and results, and whether history was open;
- the width of the side columns.

A connection created or moved while a workspace is active joins that workspace, so it stays visible.

### Edit, duplicate, delete
From the workspace menu: **Edit “name”…**, **Duplicate** and **Delete…**. Deleting a workspace leaves connections alone: only the tabs open in it are lost. [[Ctrl+Z]] undoes creating, editing and deleting.

## Workspace variables
Variables give a value to connection fields written as `${NAME}`, such as `db.${STAGE}.example.lan`.
1. Open **Edit “name”…** from the workspace menu, or the "Workspace variables…" command in the palette.
2. Under **Variables** press **Add variable** and type **Name** and **Value**.
3. Press **Save**.

- They apply while the workspace is active and win over the system environment.
- A name uses letters, digits and `_` and does not start with a digit; duplicate names or names that look like passwords block saving.
- In **All connections**, the variables of the first workspace that holds the connection apply.

How to write them in fields: [Connections and profiles](connections).

## Share folders and connections
Right-click a folder or connection › **Export…**, or **More actions** (the three dots next to **Connections**) › **Export…** for the whole workspace.

| Format | Content |
|---|---|
| **Encrypted package** | a `.nicedb` file protected by a **Passphrase** of at least 8 characters; optionally workspaces, history, appearance settings, scripts, snippets and notes, database passwords |
| **JSON definitions** | folders and connections only, in plain text, without passwords or SSH secrets |

To load them: **More actions** › **Import…**. For each item that already exists you choose what to do, such as **Keep both**, **Replace** or **Skip**: nothing is overwritten without a choice. The passphrase cannot be recovered; send it some other way. Other formats in [Import and export](import-export).

## Tabs
| Action | How |
|---|---|
| New tab on the same connection | **+** after the tabs, or [[key:new_tab]] |
| Rename | double-click the title or [[F2]]; [[Enter]] confirms, [[Esc]] gives up |
| Duplicate, close others, close tabs to the right | right-click the tab |
| Scroll through tabs | mouse wheel over the tab row |
| Reopen a closed tab | [[Ctrl+Z]] |

The tab tooltip shows the database, a running query, unapplied changes, an open transaction and production. When a connection has several databases open, the database name appears next to the title.
