# Connections and profiles

## Create a connection
The window has two parts: the list of engines on the left, the chosen engine's fields on the right, with **Test**, **Cancel** and **Save** at the bottom.

1. Press the **+** next to **Connections**, or right-click a folder › **New connection here…**.
2. Pick the engine. The cursor is already in **Find an engine**: type part of the name and the list narrows, and the first engine left becomes the choice. [[↑]] [[↓]] change engine, [[PgUp]] [[PgDn]] jump by eight, [[Enter]] moves to **Name**, [[Esc]] clears the search.
3. Type the **Name** and pick the **Folder**.
4. Fill in the connection fields (table below).
5. Pick the **Environment**: **local**, **development**, **staging** or **production**.
6. Press **Test**: the result appears next to the button. Then **Save**.

Missing required fields are highlighted on the first **Test** or **Save**.

The list groups the engines by family: relational, documents, key-value, wide-column, search, analytical, enterprise. Products that speak another engine's protocol sit indented under it (TimescaleDB, YugabyteDB, Neon and Supabase under PostgreSQL; TiDB and PlanetScale under MySQL). The search also finds an engine by its short name or a compatible product (`mssql`, `scylla`, `opensearch`, `valkey`), by family and by place: `file` shows the file-based engines, `cloud` the hosted ones.

At the top right the window repeats the chosen engine, with its family, where the data lives and, for compatible products, the protocol it uses.

**Edit** opens the connection straight on its engine, with the list put away; the button left of the mark brings it back. Once saved, the engine cannot change, except between products of the same protocol (PostgreSQL and TimescaleDB, for example): the other engines in the list stay greyed out. For another engine, create a new connection.

| Engine | Fields | Default port |
|---|---|---|
| PostgreSQL, CockroachDB | host, port, database, user, password | 5432, 26257 |
| MySQL / MariaDB | host, port, database, user, password | 3306 |
| TimescaleDB, YugabyteDB, Neon, Supabase | as PostgreSQL | 5432; YugabyteDB 5433 |
| TiDB, PlanetScale | as MySQL | 4000, 3306 |
| SQL Server | host, port, database, user, password | 1433 |
| Oracle | host, port, **Service** (or SID), user, password, **Logon privilege**; or a **Connect string** (Easy Connect or descriptor) | 1521 |
| SQLite | **Database file**, **Open read-only** | |
| DuckDB | **Database file** (empty for an in-memory database), **Open read-only** | |
| libSQL / Turso | **Location**: **Local file** (like SQLite) or **Remote** with **URL** (`libsql://`, `https://`, `wss://`) and **Access token** | |
| ClickHouse | host, port, database, user, password | 8123, 8443 with TLS |
| MongoDB | host, port, database, user, password, **Authentication database**, **Connection options**, **DNS SRV record (mongodb+srv)** | 27017 |
| Redis / Valkey | host, port, **Database (number)**, optional user, password | 6379 |

- **Choose…** next to the file opens the system file picker.
- For PostgreSQL, CockroachDB, MySQL and the [compatible engines](compatible), **Connection URL** fills host, port, database, user and password from a pasted URL (`postgresql://…`, `cockroachdb://…`, `mysql://…`); `sslmode` decides TLS. The engine chosen in the form stays when the URL fits it: a `postgresql://` pasted with CockroachDB or TimescaleDB chosen does not switch back to PostgreSQL.
- Oracle is reached by host, port and service, or by an Easy Connect string or a descriptor; the URL is `oracle://user@host:1521/service`. No Instant Client is needed. See [Oracle](oracle).
- ClickHouse goes through its HTTP interface; the native port 9000 is not used. See [ClickHouse](clickhouse).
- With **DNS SRV record** the host is the cluster name: servers and port come from DNS, and the SSH tunnel and proxy are not used. See [MongoDB](mongodb) and [Redis and Valkey](redis).

## Passwords and the keychain
Passwords, tokens, key passphrases and the passwords of proxies and SSH jumps go into the system keychain, never into the profile. When editing, the field reads **Saved in the keychain**: leaving it empty keeps the password. **Remove the saved password** deletes it.

## TLS
| Choice | Effect |
|---|---|
| **Verify certificate** | encrypted, with the server certificate checked; **CA certificate (optional)** for your own CA |
| **Encrypted, unverified** | encrypted, but nothing proves the server is the right one |
| **No TLS** | plain text |

For `localhost`, `127.*` and `::1` the choice starts at **No TLS**, for other hosts at **Verify certificate**. Neon, Supabase and PlanetScale only accept encrypted connections: **No TLS** is not offered.

**Use a client certificate** (PostgreSQL, CockroachDB, MySQL/MariaDB, SQL Server) asks for **Certificate (PEM)**, **Private key (PEM)** and **Key passphrase**. An encrypted key must be in PKCS#8 format:

```
openssl pkcs8 -topk8 -v2 aes-256-cbc -in key.pem -out key-pkcs8.pem
```

## SSH tunnel
1. Tick **SSH tunnel**.
2. Fill in **SSH host**, port and **SSH user**.
3. Pick the **Sign-in**: **Agent** (keys loaded in ssh-agent), **Key** (**Key file** and **Key passphrase**) or **Password**.
4. To go through other hosts first, use **Jump through** › **Add a jump host**, up to 8. The arrows change the order, the × removes a jump.

On the first connection **Unknown server key** appears with the fingerprint: compare it with the server's, then press **Trust and retry**. With jumps, the question comes one host at a time. A changed key blocks the connection.

## Proxy
**Go through a proxy** shows **Proxy type** (**SOCKS5** or **HTTP CONNECT**), **Proxy host**, port, **Proxy user** and **Proxy password**. The proxy carries the first leg: to the database, or to the first SSH host.

## Session
The **Session** section at the bottom of the form opens with a click; closed, it sums up its content in one line.

| Field | Effect |
|---|---|
| **Init queries** | statements run every time a session opens, after a reconnect, on a database switch and in the side connections of tools and import. If one fails, the session does not open and the error names it |
| **Keep-alive** | a `SELECT 1` on an idle session every 30 s, 1, 2, 5 or 10 minutes; never while a query runs. Network engines only |
| **Close when idle for** | after 15 min … 8 h without queries the session closes; the tab stays and the session reopens on the next query |
| **Row limit** | for `SELECT`s without `LIMIT`, `TOP` or `FETCH`; empty means the cap in **Settings › Results** |
| **Reconnect on its own when the connection drops** | reopens the session with the same profile, tunnel and password, then runs the init queries again |

Init examples: `SET search_path TO sales, public;` on PostgreSQL, `SET NAMES utf8mb4;` on MySQL, `PRAGMA foreign_keys = ON;` on SQLite.

When the connection drops, the status bar reads **connection lost** or **reconnecting…**. An interrupted query is never run again on its own: the tab offers **Run again from here**, or **Run again anyway** if it was a write. **Reconnect** forces a new attempt. An open transaction is lost with the connection.

## Variables in fields
In host, port, database, user, file, URL, certificates, SSH and proxy fields and init queries you can write `${NAME}`, also inside longer text: `db.${STAGE}.example.lan`.
- The value comes from the active workspace's variables, then from the system environment. See [Workspaces and folders](workspaces).
- `${PORT:-5432}` uses 5432 when the variable is missing; `$${` writes a literal `${`.
- Below the field you see the final value ("Becomes …") or the problem.
- Names that look like secrets (PASSWORD, TOKEN, SECRET…) are refused: passwords stay in the keychain.

## Templates
A template is a profile that never opens and sets the values of several connections, such as staging and production of the same client.
1. In the **Template** field pick **A template for other connections** and save.
2. On the template, right-click › **New connection from this template**.
3. Change only what you need: each changed field becomes **own**; ↺ **Take from the template** lines it up again.

Changes to the template reach its followers at once, except in their own fields. Name, folder, environment and password stay with each connection. In the tree a template shows its name in italics with the **template** tag; clicking it opens it for editing.

## Manage connections
Clicking a connection opens it and shows the object tree; if it is already open, it brings you to its tab. Next to the name: a mark while it opens, an error mark if opening fails (the reason is in the tooltip), and the buttons to disconnect and edit.

The right-click menu offers: **Open** or **Go to connection**, **New tab**, **Tools** (see [Tools](tools)), **Edit…**, **Duplicate…**, **Copy as URL** (no password), **Move to**, **Workspaces**, **Export…**, **Disconnect** and **Delete…**.

Deleting asks for confirmation. For 30 seconds [[Ctrl+Z]] brings the whole connection back, passwords included.

## Production
With the **production** environment the connection is marked everywhere, in the title bar and on its tabs, and starts read-only. Unlocking and confirmations are in [Production safety](production).

## Limitations
- SQL Server takes a client certificate only with a key without a passphrase.
- Proxies and SSH jumps apply to network engines; remote libSQL does not go through the proxy.
- A changed variable applies from the next open or reconnect, not to sessions already open.
- Session variables set by hand in the editor and temporary tables do not survive a reconnect.
- A restored production session starts read-only again.
