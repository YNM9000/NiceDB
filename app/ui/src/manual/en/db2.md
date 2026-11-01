# Db2

Db2 (Linux, Unix and Windows) needs IBM's CLI driver, the "clidriver": it is free, but IBM's licence does not let NiceDB ship it, so you download it once from IBM.

## IBM's driver
1. Download the clidriver for your system from IBM's public site (`odbc_cli`): `linuxx64_odbc_cli.tar.gz`, `ntx64_odbc_cli.zip`, `macarm64_odbc_cli.tar.gz` or `macos64_odbc_cli.tar.gz`. It is about 20 MB and needs no account.
2. Unpack it into a folder that stays put, for example `/opt/clidriver` or `C:\clidriver`.
3. On Windows, open an administrator prompt and run `clidriver\bin\db2cli install -setup` to register the driver.
4. In **Settings › Drivers**, choose the `clidriver` folder. If the driver is missing or the folder is wrong, the connection says so; every other engine keeps working.

## Connecting
**New connection**, **Engine** Db2: host, port (50000), **Database** (for example `GESTION`), user and password. With **TLS**, NiceDB asks the server for SSL; the **CA certificate** is the file of the server certificate. SSH tunnels and proxies work as for the other engines.

## Exploring and querying
Under the connection are the schemas, yours first; Db2's own (`SYS…`, `NULLID`, `SQLJ`) stay out. In each schema: tables with the row counts from statistics, views, materialized query tables, routines, sequences and triggers, read from `SYSCAT`. **Show DDL** for tables, views, routines, triggers, indexes and sequences; **Dependencies** from `SYSCAT.TABDEP` and the foreign keys.

Statements autocommit; `BEGIN TRANSACTION` turns that off until `COMMIT` or `ROLLBACK`. SQL PL blocks (`BEGIN … END`, procedures, triggers) stay whole. **Stop** cancels the running query.

## Explain and tools
**Explain** uses `EXPLAIN PLAN` and the explain tables of your schema: if they are missing Db2 says so, and `CALL SYSPROC.SYSINSTALLOBJECTS('EXPLAIN', 'C', NULL, CURRENT SCHEMA)` creates them. The tools use the `MON_GET_*` functions: **Activity** with locks and closing a connection (`FORCE APPLICATION`), **Monitor** (connections, transactions, size, buffer pools, costly statements), **Space** (`ADMINTABINFO`, index use) and **Users** (authorities, roles, table grants).
