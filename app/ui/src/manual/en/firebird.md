# Firebird

NiceDB speaks the Firebird protocol itself: there is no fbclient to install.

## Connecting
1. **New connection**, then pick Firebird under **Engine**.
2. **Host** and **port** (3050), then **Database**: the path of the file on the server (`/var/lib/firebird/data/sales.fdb`) or its alias.
3. **User** and **password**. The password is kept in the keychain. Firebird encrypts its own protocol: there is no TLS option.
4. SSH tunnels and proxies work as for the other engines.

## Exploring
Firebird has no schemas before version 6: everything sits in one group. There are **tables**, **views**, **procedures**, **generators** and **triggers**. From the context menu: **Show DDL** for indexes too (the index of a key becomes its `ALTER TABLE … ADD CONSTRAINT`) and **Dependencies**, read from `RDB$DEPENDENCIES` and the foreign keys.

## Queries
- Empty results still have their headers, with the type of each column.
- NUMERIC and DECIMAL come back exact (`12.50`), never rounded.
- **Stop** really cancels the query: NiceDB opens a second connection and deletes the statement from `MON$STATEMENTS`. The connection stays open.
- `SET TRANSACTION`, `COMMIT` and `ROLLBACK` open and close a transaction; everything else autocommits.

## Explain
The plan is the one Firebird picks when it prepares the statement: tables, indexes, joins, sorts. Firebird does not measure executions, so the plan is always an estimate.

## Tools
- **Activity**: the connections in `MON$ATTACHMENTS` with the statement they run; **Stop** cancels the statement, **Close** ends the connection.
- **Monitor**: connections, open transactions, database size and cache use.
- **Space**: tables with the rows estimated from the primary key statistics. Firebird does not report how much space each table takes.
- **Users**: the users in `SEC$USERS` (all of them for an administrator, only your own otherwise), roles and grants.
