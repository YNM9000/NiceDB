# Production safety

A connection whose environment is **production** opens read-only. To write you unlock writes, every write asks for confirmation, and every write goes into the log. The rules are set in **Settings › Safety**.

## Mark a connection as production
1. Right-click the connection › **Edit…**.
2. In **Environment** choose **production**.
3. **Save**.

From then on:
- the **production** stamp appears in the title bar next to the connection name;
- the **Read** / **Write** switch appears in the row below the tabs;
- the production time limit applies (see below).

The other environments (**local**, **development**, **staging**) are not locked; they only change the time limit.

## Take a connection out of production
Changing the environment from **production** to another one turns the protection off, in the sessions already open too. Saving therefore asks for the same confirmation as a write, in the “Take this connection out of production?” dialog: with **Type the name** you type the name the connection had before. The change goes into the write log (“-- environment production -> local”).

An import that replaces a production connection keeps it in production.

## Read-only
While writes are closed the switch reads **Read-only**.
- Reads run as usual: `SELECT`, `SHOW`, `EXPLAIN`, the read-only commands of MongoDB and Redis.
- A write is refused with “Production is read-only: unlock writes to run this.”.
- Where the engine allows it, the session is read-only on the server too: a write hidden inside a function is stopped by the server itself. This holds on PostgreSQL and compatibles, MySQL, MariaDB, SQLite, Firebird (read-only transactions), ClickHouse (`readonly=2` on every request) and Oracle (statements that are not queries, outside an open transaction, run in a read-only transaction). SQL Server, DuckDB, SAP HANA, Db2, Cassandra, DynamoDB and OpenSearch rely on NiceDB recognising writes.
- It covers everything that writes: editor, Data view, table designer, import, MongoDB documents, Redis keys, “Cancel query” in Tools › Activity.
- Tools › Test data never writes to a production connection.

## Unlock writes
1. In the row below the tabs, click the switch.
2. The popup says how long writes stay open and which confirmation every write will ask for.
3. **Unlock writes**.

- With timed unlocking the time left runs next to the switch: “9:42 left”. At zero writes lock again by themselves.
- If time runs out while a statement is running, the statement finishes (“time is up: locks when the statement ends”) and then writes lock.
- Clicking the open switch locks at once (**Lock now**).
- [[Esc]] or a click outside closes the popup without unlocking.
- After a reconnect the session starts read-only again: unlock again.
- **Import data** on a production connection shows the switch in its own dialog too.

## Choose how unlocking works
**Settings › Safety › Unlocking writes**:

| Mode | What happens |
|---|---|
| **Timed** | writes stay open for the **Production unlock time** (2, 5, 10, 30 or 60 minutes; 10 by default), then lock |
| **Until disconnect** | writes stay open until you close the connection, with no countdown |
| **Always open** | production is never read-only; “Writes always open” replaces the switch |

The **production** stamp stays visible in every mode, and the confirmations stay as chosen below.

Moving to a looser mode (here or in **Confirm before writing**) asks for confirmation in the “Loosen production safety?” dialog and goes into the log of every production connection.

## Confirm a write
**Settings › Safety › Confirm before writing**:

| Mode | What it asks |
|---|---|
| **Type the name** (default) | for every write you type the connection name; the button turns on only when the name matches |
| **Confirmation dialog** | a dialog with **Cancel** and **Confirm**; focus starts on **Cancel** |
| **None** | writes go straight through; only the log remains |

The confirmation dialog shows:
- the title, for example “Run on production?” or “Apply 3 changes to production?”, and the connection name;
- the statements or changes about to run;
- for risky statements, the rows they will touch (“touches 1,240 rows”) when they can be estimated;
- a warning if someone changed the rows you are about to edit in the meantime.

[[Esc]] is the same as **Cancel**. To try a write without committing it, use a dry run: see [Transactions and dry run](transactions).

## Risky statements
They ask for confirmation outside production too, with the “Run risky statements?” dialog and the **Run anyway** button:
- `UPDATE` or `DELETE` without `WHERE`, `DROP`, `TRUNCATE`, `ALTER`;
- MongoDB: `delete` or `update` on every document, `drop`, `dropDatabase`, `dropIndexes`, `collMod`, `renameCollection`;
- Redis: `FLUSHDB`, `FLUSHALL`, `KEYS`, `CONFIG SET`, `SHUTDOWN`, `CLIENT KILL` and the like;
- ClickHouse: `DROP`, `TRUNCATE`, `ALTER` including mutations.

On production they follow the chosen confirmation mode: with **None** they run without asking.

## Time limits
**Settings › Safety** has a **Time limit** for each environment. A query that runs past it is cancelled on the server with “Stopped after 60 s: the environment's time limit.”.

| Environment | Default |
|---|---|
| local | No limit |
| development | No limit |
| staging | 5 min |
| production | 1 min |

Choices: No limit, 15 s, 30 s, 1 min, 5 min, 15 min, 1 h.

## Write log
Every write, on any connection, is recorded: when, system user, database, rows touched, the text sent and any error. It covers the editor, Data view (a filter that writes too), documents, keys, the `COMMIT` of a transaction that wrote, cancelled server sessions and restores. On production, unlocking and locking writes, changing the environment and loosening the safety settings are recorded as well.

Passwords written in SQL (`PASSWORD '…'`, `IDENTIFIED BY …`, Redis `AUTH`, MongoDB `pwd` and the like) become `***` in both the log and the history.

1. Open a table in the **Data** view.
2. In its header, **Log**.
3. To save it: **Export** › **CSV table** or **JSON file**.

The panel shows the connection's last 200 writes; the export holds all of them.

## Limitations
- With manual commit, unlocking writes closes an open transaction that has not written yet: it was started read-only.
- The log lives on this computer, in NiceDB's local database: it is not a server audit trail.
- The recorded user is the operating system user, not the database user.
