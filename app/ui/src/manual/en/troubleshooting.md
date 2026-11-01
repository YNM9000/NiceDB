# Troubleshooting

## The connection does not open
1. Right-click the connection › **Edit…**.
2. **Test**: the server's or driver's message appears below the form.
3. Check in order:
   - **Host** and **Port**: for ClickHouse the HTTP port (8123, or 8443 with TLS), not 9000;
   - **User** and **Password**: an empty password field showing “Saved in the keychain” uses the saved one;
   - **Database**: it must exist and be reachable for that user;
   - network and firewall, or the **SSH tunnel** or proxy if the server is not directly reachable;
   - `${NAME}` variables in the fields: they need a value in the workspace or in the system.

If the message reads “Init statement 2 failed: …”, the connection works but one of the **Init queries** (the form's **Session** section) fails: fix or remove it.

## TLS errors
- **Verify certificate** fails with a certificate signed by an internal CA: point **CA certificate (optional)** to the CA file.
- With a self-signed certificate, or for a quick test, choose **Encrypted, unverified**: traffic stays encrypted, but nothing proves you are talking to the right server.
- **No TLS** only if the server does not offer it.
- MongoDB and Redis through an SSH tunnel or a proxy see the server as `127.0.0.1`: with verification on, the certificate must also be valid for that name.
- With **Use a client certificate**, an encrypted key must be PKCS#8 (`openssl pkcs8 -topk8`); SQL Server takes only keys without a passphrase.

## SSH server key
**Unknown key**: the first time you go through an SSH host, **Test** shows “Unknown server key” with the key's fingerprint.
1. Compare the fingerprint with the one given by whoever runs the server.
2. If it matches, **Trust and retry**.

The key is saved in the `known_hosts` file in NiceDB's data folder (see below). NiceDB also reads `~/.ssh/known_hosts`: hosts the system already knows ask nothing. The same goes for every host in **Jump through**.

**Changed key**: you see “*host*'s key changed. This could be an attack: the connection stays blocked until you verify it.”.
1. Ask whoever runs the server whether the key really changed.
2. Only if it did, remove that host's line from the `known_hosts` file that holds it (NiceDB's or `~/.ssh/known_hosts`).
3. **Test** again and trust the new key.

## Lost connection
If the network drops or the server restarts, the status bar reads “connection lost”, then “reconnecting…”. NiceDB reopens the session with the same profile, tunnel and password and runs the init queries again; a notice confirms the restore.
- If the server does not answer, retries start by themselves after 5 s, 10 s, 20 s, up to a minute. **Reconnect** forces one at once.
- An interrupted query is never run again by itself. For a read the notice says “It was a read: running it again is safe.” and offers **Run again from here**; for a write it says it may have run and offers **Run again anyway**: check the data first.
- An open transaction is lost: the notice says so.
- Variables set by hand in the editor and temporary tables do not survive.
- A production connection comes back read-only.

To stop automatic reconnects, clear **Reconnect on its own when the connection drops** in the form's **Session** section. A server that stops answering without closing the connection is noticed sooner with **Keep-alive** on. The options are described in [Connections and profiles](connections).

## Passwords and the keychain
NiceDB does not store passwords in its own files: it keeps them in the system keychain, under the service name `app.nicedb.desktop`.
- **Linux**: a running, unlocked Secret Service (GNOME Keyring, KWallet or similar).
- **macOS**: the system Keychain.
- **Windows**: Credential Manager.

An error starting with `keychain:` comes from the keychain: unlock it or start the service, then try again. If a password can no longer be found, type it again in the form and **Save**. **Remove the saved password** deletes it from the keychain.

## Production does not write
| Message | What to do |
|---|---|
| “Production is read-only: unlock writes to run this.” | unlock with the switch below the tabs |
| “Confirm with the connection name.” | confirm in the dialog that appears |
| “The typed name does not match the connection.” | type the exact connection name, case included |
| “time is up: locks when the statement ends” | the timed unlock ended: unlock again |
| “Production connection: test data is not written here.” | Test data never writes to production |

After a reconnect, unlock again. Unlock time and confirmations are set in **Settings › Safety**: see [Production safety](production).

## Queries stopped or cut short
- “Stopped after 60 s: the environment's time limit.”: the query ran past its environment's **Time limit** in **Settings › Safety**.
- “Connection limit: showing the first 1000 rows.”: **Load all** reads again without the connection's limit.
- Result marked as truncated: it reached the **Row cap per query** in **Settings › Results**.
- “The connection is already running a statement.”: wait or press [[key:cancel]], or open another tab with [[key:new_tab]].
- “A transaction is already open: commit or roll it back first.”: see [Transactions and dry run](transactions).

## Slow or incomplete schema
- The schema is read once, when the connection or database opens, and on **Refresh schema**. After changes made outside NiceDB, refresh.
- With thousands of tables the palette and completion are ready shortly after the schema arrives; meanwhile the palette says “The schema is loading…”.
- On SQL Server the first read after the server starts can take a few seconds.
- The ER diagram of thousands of tables appears later: filter by schema or name.
- Tables of a database never opened cannot be found: open the database from the drop-down.
- MongoDB and ClickHouse read the schema of the chosen database only.

## Where NiceDB keeps its data
Everything NiceDB saves lives in a per-user folder:

| System | Folder |
|---|---|
| Linux | `~/.local/share/app.nicedb.desktop` (or `$XDG_DATA_HOME/app.nicedb.desktop`) |
| macOS | `~/Library/Application Support/app.nicedb.desktop` |
| Windows | `%APPDATA%\app.nicedb.desktop` |

- `nicedb.sqlite`: connections, folders, workspaces, settings, history, write log, scripts, snippets, notes, palette marks and test-data profiles.
- `known_hosts`: the SSH keys accepted with **Trust and retry**.

Passwords are not here but in the keychain. For a backup of your connections, an `.nicedb` package is the better choice ([Import and export](import-export)). To start from scratch, quit NiceDB and move the folder elsewhere: on the next start the first-time setup appears.
