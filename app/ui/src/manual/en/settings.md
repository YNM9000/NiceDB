# Settings

## Open the settings
- Click the gear on the right of the title bar, or run “Open settings” from the palette.
- The sections are on the left: **Appearance**, **Language and sounds**, **Editor**, **Results**, **Safety**, **Keyboard shortcuts**, **Updates**, **About**. With focus on the list, [[↑]] and [[↓]] switch section.
- Every choice applies at once and is saved. **Reset**, at the top of the Editor, Results and Safety sections, restores the defaults.

## Appearance
### Style
The style changes the interface's materials, type and marks; features stay the same. Changing style replays the opening animation.

| Style | Character |
|---|---|
| **Ledger** | account book: ruled paper, pencil and ink |
| **Drawing** | technical drawing: film, title block and revisions |
| **Signal box** | railway signal box: enamel, lamps and key |
| **Price list** | trading-floor price sheet: dense figures, hairlines and signals (default) |

### Palette
Each style has three palettes, each in light and dark:

| Style | Palettes |
|---|---|
| Ledger | Linen and iron gall, Parchment and bistre, Sugar paper and charcoal |
| Drawing | India ink, Sepia, Vyco |
| Signal box | Chestnut and isabelline, Slate and mist, Carriage green and brass |
| Price list | Black and amber, Graphite and cyan (default), Midnight and ivory |

On the style and palette cards, the arrow keys pick the neighbouring choice.

### Theme and size
- **Theme**: **System** follows the operating system, or **Light** or **Dark** (default).
- **Interface size**: 90%, 100%, 110%, 125%, 150%. It scales the whole window; the editor font is set separately.
- **Review the initial setup** reopens the first-run setup (see below).

## Language and sounds
- **Language**: Italiano or English. Menus, messages and warnings change at once.
- **Sounds**: **All**, **Reduced** or **Off**. **Reduced** plays only for errors, long queries finished, writes and production; it also applies when the system asks for reduced motion.
- **Timbre**: **Marimba** or **Chimes**. The same sounds in two voices: wood or metal.
- **Volume**: 0 to 100%. **Try it** plays a cue.
- **Gesture sounds**: a tap when you change a choice or drop a dragged item. Off at first.
- **Start-up sound**: three notes when the NiceDB logotype starts flying to the title bar. Silent with **Reduced**.
- **Listen**: click a key to hear that sound.

## Editor
| Setting | Choices | Default |
|---|---|---|
| **Font size** | 11, 12, 13, 14, 15, 16, 18 | 13 |
| **Indent width** | 2, 3, 4, 8 | 2 |
| **Autocomplete** | On, Off | On |
| **Uppercase keywords when formatting** | On, Off | On |
| **Underline unknown names** | On, Off | On |
| **Error in a script** | **Stop** or **Continue** with the next statement | Stop |

See [Editor and completion](editor).

## Results
| Setting | Choices | Default |
|---|---|---|
| **Row cap per query** | 1,000, 10,000, 100,000, 1 M, 2 M | 2 M |
| **NULL values** | `NULL`, `—`, **Blank** | `NULL` |
| **Dates** | **As received**, dd/mm/yyyy | As received |
| **Numbers** | **As received**, **With thousands separator** | As received |

- Past the cap reading stops and the result is marked as truncated. A connection's own row limit, if set, applies before the cap.
- NULL, dates and numbers only change how cells look: copy, export and editing use the real value.

## Safety
- **Time limit** for each environment (local, development, staging, production): a query that runs past it is cancelled on the server.
- **Unlocking writes**: **Timed**, **Until disconnect** or **Always open**.
- **Production unlock time**: applies to timed unlocking only.
- **Confirm before writing**: **Type the name**, **Confirmation dialog** or **None**.

What each choice does: [Production safety](production).

## Change a shortcut
1. **Settings › Keyboard shortcuts**.
2. Click the shortcut to change: “Press the keys…” appears.
3. Press the new combination. [[Esc]] gives up.

- At least one modifier such as Ctrl, Alt or Shift is needed; [[Esc]] and function keys are the exception.
- If the combination is taken, “… is already used by …” appears and the old one stays.
- **Default** restores the original, **Remove** leaves the action without a shortcut (**None**), **Reset all** restores every default.
- Rows with a padlock (**Fixed**) cannot be changed: Undo, Redo, Switch workspace, Copy, Paste, Cut, Select all, Find in the editor.

Full list: [Command palette and shortcuts](palette).

## Updates
- **Installed version**: the version of this copy.
- **Check at start**: **Yes** (default) or **No**. At most once a day, silently: without a network it says nothing.
- **Check now**: checks at once. It answers “NiceDB is up to date.” or shows the new version with what's new.
- **Source**: the address NiceDB reads the published versions from.

When a new version is out, a box appears at the bottom right:
- **Update and restart** downloads it, checks the signature, replaces NiceDB and opens it again;
- **What's new** opens this section with the release notes;
- **Later** closes the box until the next start.

Every update is signed and NiceDB installs it only if the signature matches the key built into the app. If NiceDB is installed where it cannot write (for example from the .deb package), the verified file goes to the Downloads folder and the box says where: install it from there.

## About
- **Version**, **Build** (date and commit) and **System**: quote them when you report a problem.
- **Links**: source code, author and licence text, opened in the browser.
- **NiceDB licence**: GPL 3.0 or later; **Show the text** opens the full text.
- **Third-party licences**: libraries, fonts and sounds shipped in the app, grouped by licence. The field searches by name or licence (for example `mit` or `plex`); click an entry to see its licence text.
- The bunny at the bottom right opens the author's GitHub page.

## First-time setup
On first start, after the opening animation, NiceDB shows the **First-time setup** in five steps:

1. **Language**: Italiano or English.
2. **Appearance**: style, palette and theme, with a preview on the right.
3. **Comfort**: interface size, sounds, volume and editor font size.
4. **Production safety**: unlocking and confirmation. The careful options are preselected: timed unlock for 10 minutes and the connection name on every write.
5. **Ready**: the summary. **Start** opens the app, **New connection** goes straight to the connection form.

- [[Enter]] moves on, **Back** returns to the previous step; steps already seen reopen from the list on the left.
- **Skip** closes the setup; [[Esc]] does the same, after a confirmation on first start. Anything you have not chosen stays as it is.
- Every choice applies at once and can be changed later in these same sections. To see the setup again: **Settings › Appearance › Review the initial setup**.
