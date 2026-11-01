# ODBC

With the ODBC engine NiceDB connects to any database that has an ODBC driver: Informix, Teradata, Sybase, Progress, the driver of a business application.

## Connecting
1. Install the database's ODBC driver (from the vendor or your package manager).
2. **New connection**, then pick ODBC under **Engine**.
3. Choose how the driver is found:
   - **DSN**: the name of a data source already set up in `odbc.ini` or in the Windows "ODBC Data Sources" panel;
   - **Driver**: the path of the driver library (`/usr/lib64/psqlodbcw.so`) or its registered name.
4. **Connection string**: the driver's keywords, for example `Server=db.lan;Port=5432;Database=sales`.
5. **User** and **password**; the password is kept in the keychain and reaches the driver as `PWD`.

## What works
Queries and scripts, results, the Data view with edits, transactions (`BEGIN TRANSACTION`, `COMMIT`, `ROLLBACK`), Stop, the tree with schemas, tables and views, "Show DDL" for tables, schema and row compare, import and export, value search and test data.

## Limits
NiceDB does not know the database behind the driver: no Explain, Activity, Monitor, Space or Users. Names in mixed case or with special characters go in double quotes; with a driver that quotes differently, keep names simple.
