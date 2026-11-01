# Notes, scripts and snippets

## Open the panel
The **Notes, scripts and snippets** panel sits on the right of the tab. It opens and closes with the notebook button on the tab row or with [[key:panel_notes]]. Its tabs are **Notes**, **Scripts** and **Snippets**; with the tabs focused, [[←]] and [[→]] move between them. From the palette: "Open scripts", "Open snippets".

Drag the panel's left edge to widen it. While it is open, the side **History** stays hidden.

## Notes
A note is Markdown text tied to a connection or to a workspace.

1. Open **Notes**.
2. Pick what it is about: **Connection** (the tab's connection) or **Workspace** (the active workspace, or **All connections**).
3. Write. Headings with `#`, lists with `-`, code between triple backticks.

- The note saves itself as you type; the top line reads **Saving…**, **Saved** or, if something goes wrong, **Not saved**.
- **Preview** shows the rendered Markdown, **Write** goes back to the text.
- **Search notes** looks through every note; clicking a result opens that note.

## Scripts
A script is a saved query with a name, folder and tags.

### Save a script
1. In the editor, select the text to save, or leave everything unselected.
2. Press **Save script** in the query bar, or [[key:save_script]].
3. Check **Name** (it starts from the first line), **Folder** and comma-separated **Tags**.
4. Under **Applies to**, pick the tab's connection or **Any connection**.
5. Press **Save**.

A folder is written as a path, such as `Reports/Billing`. If the text has parameters, the script keeps their values too: see [Query parameters](parameters).

### Use scripts
The list shows the scripts of the tab's connection and those for any connection; **All connections** shows them all. **Search scripts** looks at name, folder, tags and SQL.

| Action | How |
|---|---|
| Open in a new tab | click the script |
| Open and run | the **Run** button on the row |
| Insert at the cursor | the **Insert into editor** button (Query view only) |
| Rename | [[F2]] or **Actions** › **Rename** |
| Move | **Actions** › **Move to**, also into a **New folder…** |
| Change name, folder, tags and connection | **Actions** › **Edit…** |
| Replace the text with the editor's | **Actions** › **Update from the editor** |
| Delete | [[Del]] or **Actions** › **Delete**; [[Ctrl+Z]] brings it back |

A script tied to another connection opens on that connection. Scripts can also be found with **Search** in the title bar.

## Snippets
A snippet is a piece of SQL with placeholders, inserted by typing a word.

### Insert a snippet
- In the editor, type the snippet's word (for example `topn`, `countby`, `dups`, `join`, `cte`): it shows in the completion list with the **§** mark; [[Enter]] replaces it with the text.
- Or, under **Snippets**, click the snippet: it goes in at the cursor.

The cursor lands on the first highlighted placeholder: [[Tab]] moves to the next, [[Shift+Tab]] goes back, [[Esc]] leaves. A repeated placeholder is typed once.

### Create a snippet
1. Under **Snippets** press **New snippet**.
2. Fill in **Name**, **Word to type** (letters, digits and `_`), **Engine** (**All** or a single one), **Description** and **Body**.
3. Write placeholders in the body as `${1:table}`, `${2:column}`; the same number in two places fills in together.
4. Press **Save**.

The list separates **My snippets** from the **Built in** ones for the tab's engine. A built-in snippet cannot be edited: right-click › **Copy to mine**, then change the copy. The right-click menu also has **Open in a new tab**, **Copy**, **Edit…** and **Delete**.

```sql
select ${1:column}, count(*) as n
from ${2:table}
group by ${1:column}
order by n desc
```

## Where they live
Notes, scripts and snippets live in the app's local database. They travel in a `.nicedb` package when you tick "Scripts, snippets and notes" on export: see [Workspaces and folders](workspaces).
