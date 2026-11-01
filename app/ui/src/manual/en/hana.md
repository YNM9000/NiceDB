# SAP HANA

NiceDB speaks the SAP HANA protocol itself: there is no SAP client to install. This engine has not been tried on a real server yet.

## Connecting
1. **New connection**, then pick SAP HANA under **Engine**.
2. **Host** and **port**: 30015 for a single-database system, the SQL port of the tenant for a multitenant one (for example 30041).
3. **Tenant**: the name of the database in a multitenant system; leave it empty for the database the port belongs to.
4. **User** and **password**; the password is kept in the keychain.
5. **TLS**: with a CA certificate, point to the file and NiceDB uses its folder; without one, the common certificate authorities apply.

## Exploring
Under the connection are the schemas, yours first; system schemas stay out. In each schema: tables (with their rows), views, procedures and functions, sequences and triggers. **Show DDL** uses HANA's `GET_OBJECT_DEFINITION`.

## Queries and transactions
Statements autocommit. `SET TRANSACTION` opens a transaction, `COMMIT` or `ROLLBACK` ends it. DDL commits on its own. **Stop** cancels the query from a second connection (`ALTER SYSTEM CANCEL SESSION`).

## Explain and tools
**Explain** uses `EXPLAIN PLAN` and draws the operators with their estimated rows and costs. The tools read the `M_` views: **Activity** (connections, active statements, locks), **Monitor** (connections, transactions, data size, costly statements from the plan cache), **Space** (table sizes) and **Users** (users, roles and privileges).
