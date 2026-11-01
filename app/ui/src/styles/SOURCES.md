# Fonts

All fonts are licensed under the SIL Open Font License 1.1 and bundled as woff2 latin subsets in `fonts/`, declared once in `fonts.css`. The PDF reports embed the same subsets as TTF (`crates/nd-pdf/fonts/`, decompressed with fontTools; Source Serif 4 instanced at weight 600).

- IBM Plex Sans 400, 400 italic, 500, 600 — UI. © IBM Corp. https://github.com/IBM/plex
- IBM Plex Sans Condensed 400, 500, 600 — condensed UI and display (Tavola, Banco). © IBM Corp. https://github.com/IBM/plex
- IBM Plex Mono 400, 400 italic, 500, 600 — code and data. © IBM Corp. https://github.com/IBM/plex
- Source Serif 4, variable (wght 200–900, opsz 8–60), roman and italic — display serif and the wordmark. © Adobe. https://github.com/adobe-fonts/source-serif — files from Fontsource (`@fontsource-variable/source-serif-4` 5.2.9).

# Icons

- UI: Lucide (ISC). The chevron of disclosure summaries in `controls.css` is Lucide's `chevron-right` path.
- Control marks (check ticks, the drafting cross, carets): drawn for NiceDB as inline SVG in `controls.css`.
- Database engines: Simple Icons (CC0 data; marks are trademarks of their owners, used only to identify the engine). SQL Server and Oracle are not in Simple Icons: monograms.

# Sounds

See `../assets/sounds/SOURCES.md`.
