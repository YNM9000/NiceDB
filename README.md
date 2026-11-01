<p align="center">
  <img src=".github/readme/banner.png" alt="NiceDB: made for production databases; relational, documents, key-value, wide-column, search and analytical engines; runs on Linux, Windows and macOS" width="100%">
</p>

**NiceDB is a desktop database client built around one rule: production is read-only until you decide otherwise.**

It connects to PostgreSQL, MySQL, SQL Server, Oracle, MongoDB, Redis, ClickHouse and the other engines listed below, and it is made for the people who open those connections every working day: the software house that looks after its customers' servers, the company whose business system lives in a database. Free software under the GPL, written in Rust with Tauri 2 and Svelte 5.

<p align="center">
  <img src=".github/readme/production.gif" alt="Editing a production table: read-only at first, unlocked for ten minutes, one cell changed, the connection name typed to apply, the UPDATE in the write log" width="100%">
</p>

## Writing to production

Four things stand between you and a production table, in this order.

1. **The stamp.** A production connection is marked wherever it appears: tree, tab, title bar, status line. It opens read-only.
2. **The unlock.** Writing opens for ten minutes, with the time left in view, then locks again by itself. You can choose to keep it open until you disconnect.
3. **The name.** Before a change is applied, NiceDB shows what it touches and asks you to type the connection's name. `UPDATE` and `DELETE` without `WHERE`, `DROP` and `TRUNCATE` ask before they run.
4. **The log.** Every statement that changed something is kept, with who, when, where and how many rows.

Every write passes through one place in the core, so no screen can skip a step. Each one is set in Settings, under Safety.

<p align="center">
  <img src=".github/readme/safety.webp" alt="Settings, Safety: time limits per environment, how writes unlock, what each write asks" width="100%">
</p>

## A working day

<p align="center">
  <img src=".github/readme/plan.gif" alt="A query typed and explained: the plan drawn with its slowest step marked, the step opened, then the same plan as a list" width="100%">
</p>

**Ask why a query is slow.** Explain draws the plan: line width is rows, the marked track is where the time goes, and the notes name the missing index with the `CREATE INDEX` to copy. Plans are read for PostgreSQL, MySQL, MariaDB, SQL Server, Oracle, Db2, SAP HANA, Firebird, SQLite, DuckDB, CockroachDB, TiDB, ClickHouse, MongoDB, Cassandra and Elasticsearch.

<table>
  <tr>
    <td width="50%"><img src=".github/readme/activity.webp" alt="Activity: sessions, a blocked UPDATE and the open transaction holding the lock"></td>
    <td width="50%"><img src=".github/readme/er.webp" alt="ER diagram with one table selected and its relations drawn"></td>
  </tr>
  <tr>
    <td><b>See who blocks whom.</b> Sessions, locks and slow queries, and the session to close.</td>
    <td><b>Read the schema.</b> A selected table draws its relations and lists what points to it.</td>
  </tr>
  <tr>
    <td colspan="2"><img src=".github/readme/documents.webp" alt="A MongoDB collection as documents, the selected one as a typed tree"></td>
  </tr>
  <tr>
    <td colspan="2"><b>Open any kind of database.</b> A MongoDB collection opens as documents, a Redis database as keys, an Elasticsearch index takes console requests. The data grid, the editor and the tools adapt to what each engine can do.</td>
  </tr>
</table>

The rest of the day is there too: completion that knows the schema, editing in the grid with a dry run before commit, foreign keys to follow, schema and data comparison, test data, backups with each engine's own tools, CSV, JSON, Excel and Parquet import and export, a security check of the server's configuration.

## Engines

```text
relational    PostgreSQL · MySQL · MariaDB · SQL Server · Oracle · Db2 · SAP HANA
              Firebird · CockroachDB · SQLite · libSQL and Turso
compatible    TimescaleDB · YugabyteDB · Neon · Supabase     PostgreSQL protocol
              TiDB · PlanetScale                             MySQL protocol
documents     MongoDB
key-value     Redis · Valkey
wide-column   Cassandra · ScyllaDB · DynamoDB
search        Elasticsearch · OpenSearch
analytical    DuckDB · ClickHouse
others        any database with an ODBC driver
```

<p align="center">
  <img src=".github/readme/connection.gif" alt="A new connection: PostgreSQL picked, a URL pasted, the fields filled in, marked as production, tested, saved" width="100%">
</p>

Paste the URL from your provider and the form fills itself; the password goes to the keychain. SSH tunnels with jump hosts, SOCKS5 and HTTP proxies, client certificates and per-workspace variables are in the same form.

## Four styles

<p align="center">
  <img src=".github/readme/styles.webp" alt="The same production table in the four styles, dark: Price list, Ledger, Drawing, Signal box" width="100%">
</p>

| Style | Modelled on |
| --- | --- |
| **Price list** | A trading-floor price sheet: dense figures, hairlines and signals. The default. |
| **Ledger** | An account book: ruled paper, pencil and ink. |
| **Drawing** | A technical drawing: film, title block and revisions. |
| **Signal box** | A railway signal box: enamel tiles, lamps and keys. |

One set of controls, drawn four ways. Each style has three palettes, light and dark, and follows the system theme.

## Figures

Measured on a catalog of 20,000 tables and on local servers.

| Measure | Time |
| --- | ---: |
| Interface ready after a cold start | 210 ms |
| First rows on screen, from the server's first byte | 36–55 ms |
| Completion, from key to list | 45 ms |
| Scrolling 1.2 million rows | 60 fps |
| Catalog of 10,000 PostgreSQL tables | 186 ms |
| Catalog of 8,000 SQL Server tables, in one round trip | 0.41–0.50 s |

## Where your data stays

Profiles stay in a local database. Passwords, tokens and key passphrases go to the system keychain, never into a profile or a log. There is no account and no telemetry. The one request NiceDB makes by itself is a daily update check, which you can turn off; updates are signed and installed only when the signature matches the key built into the app.

## Status

NiceDB is at version 0.1, the first public release. It is tested against local servers for most of the engines above, plus SAP HANA Cloud. Bug reports and feature requests are welcome in the issues.

## Platforms

- **Linux** x86_64: binary, AppImage and `.deb`, for distributions with glibc 2.35 or newer and WebKitGTK 4.1.
- **Windows** x86_64: a single `.exe` or a per-user installer, on the system WebView2.
- **macOS**: a universal `.app` for Apple silicon and Intel.

Binaries are not code-signed yet, so SmartScreen and Gatekeeper will ask once before the first start.

## Build from source

You need:

- **Rust 1.98.0**, pinned in `rust-toolchain.toml`: rustup installs it by itself on the first `cargo` command.
- **Node.js 22.12** or newer and **pnpm 11** (the exact version is the `packageManager` field of `app/ui/package.json`).
- A C and C++ compiler, CMake and pkg-config.
- On Linux, the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) (WebKitGTK 4.1) and the D-Bus headers.

On Debian and Ubuntu:

```sh
sudo apt install build-essential cmake pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
  libxdo-dev libssl-dev libdbus-1-dev
```

On Fedora:

```sh
sudo dnf install gcc-c++ cmake pkgconf-pkg-config webkit2gtk4.1-devel gtk3-devel libsoup3-devel \
  javascriptcoregtk4.1-devel libappindicator-gtk3-devel librsvg2-devel openssl-devel dbus-devel
```

Then, from the repository root:

```sh
pnpm -C app/ui install --frozen-lockfile
pnpm -C app/ui build                    # the interface, embedded into the binary
cargo build --release -p nicedb-app     # target/release/nicedb
```

The first build compiles SQLite, DuckDB and unixODBC from source, so it takes a while and needs a few GB of disk.

To work on the app with the interface reloading as you edit it:

```sh
cd app/src-tauri && ../ui/node_modules/.bin/tauri dev
```

The interface also runs in a browser against an in-memory backend with made-up connections and data, which is where the pictures on this page come from:

```sh
pnpm -C app/ui dev                      # http://localhost:5183/?demo=1&locale=en&style=listino&theme=dark
```

`style` takes `listino`, `registro`, `tavola` or `banco`; `theme` takes `light` or `dark`. Release packages for all three systems are made from Linux with `scripts/build-all.sh linux windows macos`, which builds Linux in a container and cross-compiles the other two.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the checks to run before a pull request, and [SECURITY.md](SECURITY.md) to report a vulnerability.

## Inside

A Rust core (`crates/nd-core`) talks to one crate per engine (`crates/nd-drv-*`) through a common driver contract (`crates/nd-driver`). SQL dialects, quoting and statement splitting live in `crates/nd-sql`, plan readers in `crates/nd-plan`. The Tauri 2 shell is `app/src-tauri`; the Svelte 5 interface, its Italian and English translations and the built-in manual are `app/ui`.

## License

GNU General Public License, version 3 or later, with an additional permission under section 7 for elkjs and the WebView2 loader: see [LICENSE](LICENSE). The bundled fonts, IBM Plex and Source Serif 4, are under the SIL Open Font License. Engine marks come from Simple Icons, devicon, Wikimedia Commons and Material Design Icons and belong to their owners; they only identify the engine. Third-party code keeps its own license, and the full list is in the app under Settings, About.
