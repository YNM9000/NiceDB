# Redis and Valkey

Redis and Valkey share the same driver. NiceDB recognises Valkey from the server's info and names it as such in the status bar.

## Connect
1. **New connection**, then choose **Redis / Valkey** in **Engine**.
2. Fill in **Host** and **Port** (usually 6379).
3. **Database (number)**: the logical database to open, from 0 up.
4. **User**: an ACL user; empty means the `default` user, as on servers with only `requirepass`.
5. **Password**: empty for a server without one. It is kept in the keychain.
6. **TLS**, **SSH tunnel** and proxy work as for the other engines. **Test**, then **Save**.

**Mode** above the host picks **Single server**, **Cluster** or **Sentinel**. With Sentinel you give one or more Sentinels (`s1:26379, s2:26379`) and the **Master name**: NiceDB connects to the master they point to. In the explorer, under the connection, a line says which master is in use. When the Sentinels move it (failover), the old master becomes a replica: NiceDB notices at the first refused command or at the keep-alive, reconnects to the new one, says so in a notice, and the line shows the time of the failover and the previous master. **Tools › Server** has a **Sentinel** section with the master, the replicas and how many Sentinels watch it.

`${NAME}` variables work in every field, including the database number (`${DB:-0}`). Each connection introduces itself to the server as `NiceDB` in `CLIENT LIST`.

## Browse keys
Under the connection are the databases (`db0`, `db1`…) and, in the open database, the **prefixes**: the part of the name before the colon, with how many keys each holds. Switch database from the drop-down above the explorer.

Clicking a prefix opens the key browser filtered on `prefix:*`. The browser is the **Data** view of Redis connections: keys on the left, the value on the right.

1. In **Filter keys** type a Redis pattern (`cache:user:*`, `session:web:0?d*`) and pick the type in the drop-down.
2. [[Enter]] reads again from the start.
3. Keys are grouped on colons into folders; each key shows its type, expiry and memory.
4. NiceDB reads 1000 keys at a time with `SCAN`, never `KEYS`. The footer says how many keys have been read out of the database total: **Load more** reads the next page, **Load all** goes on to the end (at most 200,000), **Stop** interrupts.

| Key | Effect |
|---|---|
| [[↑]] [[↓]] | previous or next key |
| [[→]] [[←]] | opens or closes a folder |
| [[Enter]] | opens the key |
| [[Del]] | deletes the key |

Right-click a folder for **Show only this folder** and **Copy prefix**. Right-click a key for **Copy name**, **Read in the editor** (opens a tab with `HGETALL`, `LRANGE`, `XRANGE`… ready to run) and **Delete key**.

## Read and edit a value
The value header shows name, type, length, memory and expiry, with buttons to copy the name, **Read the value again**, **Rename**, **Change expiry** and **Delete key**.

| Type | Shown as | What you can change |
|---|---|---|
| String | text; JSON also as a tree | the text, with **Save** or [[Ctrl+Enter]]; the expiry stays |
| Hash | field and value | the value (double-click, [[Enter]] saves), new fields, remove a field |
| List | index and value | the value, add **At the head** or **At the tail**, remove an item |
| Set | the members | add and remove members |
| Sorted set | member and score, highest first | the score, new members, remove a member |
| Stream | ID and fields | new entries (one field per line: `status shipped`), remove an entry |
| JSON | tree or text | the whole document, if the RedisJSON module is there |

- Large collections arrive in pages of 200; “load more” sits at the bottom of the table.
- Strings over 1 MB show only the first MB and cannot be edited; binary values are read-only, shown with `\xNN` sequences.
- Removing a list item first checks that the index still holds the value that was read.
- **Rename** never overwrites a key that already exists.
- **Expiry**: type seconds or a unit (`90`, `15m`, `2h`, `7d`) and press **Set**; **No expiry** removes it.
- **Delete key**, outside production, needs a second click: “Press again to delete”.

## Create a key
1. **New key** (the `+` at the top) or the same command in the palette.
2. Type the **Key name**, choose the **Type**, the first value and the **Expiry** (empty: never expires).
3. **Create**. If the key already exists, NiceDB stops.

## Write commands
The editor takes Redis commands, one per line, as in `redis-cli`: double-quoted arguments with `\n` and `\xNN`, single-quoted ones taken literally. Lines starting with `#` or `//` are comments.

```
# who bought the most
ZREVRANGE ranking:customers:revenue 0 9 WITHSCORES
HGETALL sessione:app:tablet-01
SET cache:pricelist:note "updated at 9" EX 600
```

- [[key:run_statement]] runs the line under the cursor, [[key:run_script]] runs them all in order; [[key:format]] uppercases command names.
- Autocomplete offers commands, subcommands (`CONFIG GET`, `CLIENT LIST`…) and the names of keys starting with what you type.
- Every reply becomes a grid: a single value, a list with index and value, a map with field and value, pairs (`WITHSCORES`) in columns.
- [[key:cancel]] also stops commands that wait, such as `BLPOP … 0`.
- These do not run from the editor: `SELECT` (use the database drop-down), `SUBSCRIBE`, `MONITOR`, `MULTI`/`EXEC`, `WATCH`, `HELLO`, `RESET`, `QUIT`.

## Production and the write log
Read commands (`GET`, `HGETALL`, `SCAN`, `INFO`, `CONFIG GET`…) run even while writes are locked; all others need writes unlocked. `FLUSHDB`, `FLUSHALL`, `KEYS`, `CONFIG SET`, `SHUTDOWN`, `DEBUG`, `CLIENT KILL`, `SWAPDB`, `REPLICAOF` ask for confirmation as risky. Edits from the browser show the commands about to run in the confirmation and are logged as `redis-cli` would write them. See [Production safety](production).

## Server panel
**Tools › Server** shows version, uptime, commands per second, clients, memory, keys and read hit rate. Below are four parts:
- **Info**: the `INFO` sections and the most called commands; **Filter** searches names and values.
- **Clients**: `CLIENT LIST`, with NiceDB's own row highlighted.
- **Slow log**: `SLOWLOG GET 128`.
- **Memory**: `MEMORY STATS` and the advice of `MEMORY DOCTOR`.

If the user lacks the privilege for a part, the panel says so in its place.

In the command palette, with a Redis tab in front: “New key”, “Read the keys again”, “Load all”, “Open Server”.

## Limitations
- Folder counts cover the keys read so far until the scan is complete.
- Explorer prefixes come from at most 20,000 keys: on huge databases a rare prefix may be missing.
- After a failover the command that hit the replica is not repeated: run it again.
- Keys with non-UTF-8 bytes in the name cannot be opened from the browser; they can from the editor, in quotes.
- No client certificate for mutual TLS. Through a tunnel or proxy the server is seen as `127.0.0.1`: with **Verify certificate** the certificate must also be valid for that name.
- Memory of large collections is an estimate.
- Modules other than RedisJSON are used from the editor only.
