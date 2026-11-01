# Oracle

Oracle works like the other SQL engines, with its own dialect and PL/SQL. NiceDB speaks Oracle's protocol itself: there is no Instant Client or anything else to install.

## Connect
1. **New connection**, then pick Oracle as the **Engine**.
2. **Address**:
   - **Host and service**: host, port (1521) and **Service**, for example `FREEPDB1` or `ORCLPDB1`. If the database is reached by SID, tick **The name is a SID, not a service**.
   - **Connect string**: an Easy Connect string (`db.example.lan:1521/SALES`) or a whole `(DESCRIPTION=…)` descriptor, as in `tnsnames.ora`. With a connect string the SSH tunnel and the proxy are not used.
3. **User** and **password**. The password is kept in the keychain.
4. **Logon privilege**: **Normal**, **SYSDBA** or **SYSOPER**.
5. **TLS**: with TLS NiceDB uses TCPS, usually on port 2484. The **CA certificate** becomes the only trusted root; **Encrypted, unverified** skips the name check, but the certificate chain is still verified.
6. **Test**, then **Save**.

**Copy as URL** from the right-click menu of the connection gives `oracle://user@host:1521/service`; pasting the same URL into the form fills it in.

Kerberos, wallets with stored passwords, external authentication and native network encryption (NNE) are not available: use TLS to encrypt.

## Session
Every session starts with dates and numbers in a format that reads back without ambiguity: `YYYY-MM-DD HH24:MI:SS` for dates, `.` as the decimal separator. The connection's init queries run afterwards, for example:

```sql
ALTER SESSION SET CURRENT_SCHEMA = SALES;
ALTER SESSION SET TIME_ZONE = 'Europe/Rome';
```

## Explore
Under the connection are the schemas: the user's own first, then the ones that hold objects and are not Oracle's (`SYS`, `SYSTEM`, `XDB` and the others are left out). In each schema:
- **Tables**, with the rows estimated by the statistics; expanded, the columns with their type (`NUMBER(12,2)`, `VARCHAR2(80 CHAR)`…), primary key and indexes.
- **Views** and **materialized views**.
- **Functions** and procedures with their parameters, **Packages**, **Sequences**, **Synonyms** with the object they point to, **Types** and **Triggers** with their table.

Right-click › **Show DDL** asks `DBMS_METADATA.GET_DDL` for the text: for a table its indexes and comments too, for a package both specification and body.

## The editor
- SQL statements are separated by `;`, as elsewhere; NiceDB drops the `;` before sending them, because Oracle refuses it.
- PL/SQL blocks (`BEGIN … END;`, `DECLARE …`) and the `CREATE` of procedures, functions, packages, types and triggers stay whole with their `END;`. A line holding only `/` ends a block, as in SQL*Plus; after a type or a compound trigger it is required.
- `EXEC procedure(1)` becomes `BEGIN procedure(1); END;`.
- `q'[…]'` strings are understood, `:name` placeholders are filled as in other queries; `:new` and `:old` in triggers are not placeholders.
- The `block` macro writes an anonymous block.

## Transactions
Oracle never commits by itself on the wire: NiceDB commits after each statement while the tab is in automatic commit. In manual or smart commit the transaction starts with `SET TRANSACTION READ WRITE` and waits for **Commit** or **Rollback**. Every DDL statement (`CREATE`, `ALTER`, `DROP`, `TRUNCATE`, `GRANT`…) commits the open transaction: the tab says so.

Mind that in Oracle an empty string is `NULL`. Clearing a text cell in the Data view writes `NULL`.

## Data view
Rows are changed by primary key or by a unique index. A table without either uses its `ROWID`: it appears as the last column and only serves to find the row again. `CLOB`, `BLOB` and `JSON` cells can be edited, but they do not help tell a row changed by someone else; `RAW` is written in hex (`\xCAFE`).

## Execution plan
**Explain** runs the statement with `STATISTICS_LEVEL = ALL` and reads `DBMS_XPLAN.DISPLAY_CURSOR` with `ALLSTATS LAST`: actual rows, times and reads per step. With **Estimate only** it uses `EXPLAIN PLAN` and reads the table `DBMS_XPLAN.DISPLAY` prints, running nothing. Real timings need read access to `V$SESSION`, `V$SQL` and `V$SQL_PLAN_STATISTICS_ALL` (`SELECT_CATALOG_ROLE`, for instance); without it the plan stays an estimate and says so. A measured write is undone at the end, as on the other engines.

## Stopping a query
- With the `ALTER SYSTEM` privilege, **Cancel** closes the session on the server if the query has not stopped within a second and a half; NiceDB opens a new one right away and says so. Oracle's Rust driver cannot yet recover after an `ALTER SYSTEM CANCEL SQL`, so closing is the only clean way. With a transaction open the session is not closed.
- Without the privilege, **Cancel** stops the rows at the next batch and the tooltip on the button explains it; work the server has started goes on until it finishes.

## Tools
- **Activity**: the sessions of `V$SESSION` with their statement, wait and who blocks whom. A session's id is `sid,serial#`. **Cancel query** uses `ALTER SYSTEM CANCEL SQL`, **Close session** `ALTER SYSTEM KILL SESSION … IMMEDIATE`, always through the production gate.
- **Monitor**: sessions, open transactions, data file size, cache read ratio, **Long operations** from `V$SESSION_LONGOPS` and the costliest statements from `V$SQLAREA`. Without `SELECT_CATALOG_ROLE` some parts say what is missing.
- **Space**: tables and indexes from `DBA_SEGMENTS`, or only the own schema from `USER_SEGMENTS`.
- **Users and roles**: users, roles and privileges; create, grant and revoke with a preview of the SQL. A new user starts without a password (`NO AUTHENTICATION`): give it one with `ALTER USER … IDENTIFIED BY …`.
- **Maintenance** (right-click a table or schema): **Update statistics** with `DBMS_STATS`.
- Backups are not made from here: use Data Pump (`expdp`, `impdp`) or RMAN on the server.
