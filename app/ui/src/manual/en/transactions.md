# Transactions and dry run

This works on PostgreSQL, CockroachDB, MySQL, MariaDB, SQL Server, SQLite, libSQL and DuckDB.

## Commit mode
The button on the right of the tab row shows the mode of the active tab's session (**Auto**, **Manual**, **Smart**). Click it to open the menu:

| Mode | Behaviour |
|---|---|
| **Auto-commit** | each statement commits by itself (the default) |
| **Manual commit** | a transaction opens before the first statement; everything waits for **Commit** or **Rollback** |
| **Smart commit** | automatic while you read; the first write opens the transaction, which stays until Commit or Rollback |

- The mode belongs to the session: tabs on the same connection and database share it.
- It survives a reconnect. Changing it does not close a transaction that is already open.
- A `BEGIN` typed in the editor opens a transaction even in auto mode; `COMMIT` and `ROLLBACK` typed by hand close it, and the bar follows.

## The open transaction
With a transaction open, **Transaction open** appears next to the mode with writes, rows and duration, such as "3 writes · 128 rows · 1:24", and the **Rollback** and **Commit** buttons. It shows in every view.

- **Rollback** is always allowed, even with production locked.
- **Commit** on production asks to unlock writes: see [Production safety](production).
- On PostgreSQL, CockroachDB and DuckDB an error leaves the transaction unusable: the indicator reads "failed: rollback only" and **Commit** is disabled.
- MySQL and MariaDB commit the transaction by themselves before a DDL statement (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`…): the tab says so and the indicator goes off.
- Statements the engine refuses inside a transaction (`VACUUM`, `CREATE DATABASE`, `CREATE INDEX CONCURRENTLY`, `ATTACH`…) run outside it, without opening one.

In the Data view, with manual or smart commit or with a transaction open, **Apply** does not commit: the changes join the transaction and wait for **Commit**. See [Data view and editing](data).

## Dry run
A dry run executes a write inside a transaction, shows the rows it touches and waits for your decision.

1. Put the cursor on the statement, or select several statements.
2. Press **Dry run** in the query bar or [[key:dry_run]], or right-click in the editor › **Dry run**.
3. Look at the **Before and after** tab: in updates the old value is struck through with the new one beside it, in deletes the rows are struck through, in inserts you see the new rows.
4. Above the results: "Dry run: 1 write · 300 rows. Nothing is kept until you press Commit."
5. Press **Commit** to keep the changes, **Rollback** to throw them away.

![Dry run of a write](anim:dry-run)

- The first 200 rows per statement are shown; the count is always the real one.
- For an `UPDATE` the new values come from rehearsing the statement in a savepoint that is rolled back right away: you see `now()`, defaults and columns changed by triggers. Where that is not possible (DuckDB, remote libSQL, tables without a primary key or on MyISAM) they are an estimate and the tab says so below the table.
- `INSERT`s show the inserted rows on PostgreSQL, CockroachDB, SQLite, libSQL and DuckDB; on MySQL, MariaDB and SQL Server only the count.
- If a statement fails, or you stop it with **Cancel**, the dry run rolls back by itself.
- A dry run does not ask to confirm risky statements: you see the rows before committing.
- On production, writes must already be unlocked for the dry run, because rows are really touched and locked until you close it; the connection name is asked at **Commit**.

Transaction statements (`BEGIN`, `COMMIT`…), statements the engine refuses inside a transaction and, on MySQL and MariaDB, DDL cannot be dry-run. There must also be no transaction already open. In those cases the dry run does not start and the tab names the statement that blocks it.

## Close with a transaction open
Closing the last tab of a session, disconnecting, switching workspace or quitting while a transaction holds writes opens a dialog:

| Button | Effect |
|---|---|
| **Don't close** | leaves everything as it is |
| **Roll back and close** | rolls back, then closes |
| **Commit and close** | commits, then closes; on production it goes through unlocking |

A transaction without writes closes without asking.

## Dropped connection and idle close
The transaction lives in the connection: if the connection drops, the server rolls it back. The tab always tells you:
- dropped during a query: "The server rolled back the open transaction";
- connection restored by keep-alive or **Reconnect**: the notice says the transaction was rolled back;
- session reopened on first use: the transaction is gone, with its writes;
- idle close: the transaction is rolled back and the notice says so.

The commit mode stays as it was. Session options are in [Connections and profiles](connections).

## Ctrl+Z
[[Ctrl+Z]] never touches transactions: it does not undo a Commit or a Rollback. Reopening a closed tab with [[Ctrl+Z]] does not bring back the transaction that closed with it.

## Limitations
- The preview of an `UPDATE` computes new values with a `SELECT` before running: `now()` or `random()` can give slightly different values from those written, and triggers are not shown.
- For `UPDATE … FROM`, joins, `MERGE`, grouped assignments, `ORDER BY`/`LIMIT` in writes and `UPDATE TOP` you see only the count.
- The preview gets 5 seconds; if that is not enough, the dry run goes on without it.
- SQL Server can roll back a transaction by itself after some severe errors; the indicator finds out at Commit. A `BEGIN` inside a SQL Server batch or procedure is not seen by the bar.
- DuckDB has no savepoints: with a transaction open, **Apply** in the Data view asks you to close it first.
- On production with manual commit, unlocking writes closes an open transaction that has no writes yet.
