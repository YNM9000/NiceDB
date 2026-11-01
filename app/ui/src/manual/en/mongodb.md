# MongoDB

## Connect to MongoDB
1. **New connection**, then choose MongoDB in **Engine**.
2. Fill in **Host**, **Port** (usually 27017), **User** and **Password**. An empty user means a server without authentication.
3. If needed, fill in the MongoDB fields:
   - **DNS SRV record (mongodb+srv)**: the host is the cluster name (`cluster0.ab12c.mongodb.net`); servers and port come from DNS. With SRV the SSH tunnel and the proxy are not used.
   - **Authentication database**: where the user is defined; empty means `admin`.
   - **Connection options**: the part after `?` of a URI, for example `replicaSet=rs0&readPreference=secondaryPreferred`.
4. In **TLS** choose **Verify certificate** (with a **CA certificate (optional)**), **Encrypted, unverified** or **No TLS**.
5. **Test**, then **Save**.

The password lives in the system keychain. Through an SSH tunnel or a proxy NiceDB connects to the given server only, even if it belongs to a replica set. `${NAME}` variables work in every field.

## Browse databases and collections
Under the connection are the databases; open one to see its **collections** and **views**. Each collection shows the estimated document count, its fields and its indexes.

- Fields are inferred from a sample of 100 documents. Each field shows its type; with mixed types, the share of each, for example `decimal 98% · string 2%`.
- Nested fields use dots: `address.city`, `lines.item`.
- Clicking a collection opens a tab with a `find` on the first 100 documents and runs it.

Right-click a collection for **Open the documents**, **New query here**, **Show the create command** (options and indexes, ready to run again), **Copy name**, **Copy the find command**.

## Write commands
The editor takes MongoDB database commands in JSON, the same as `db.runCommand`:

```json
// customers in Ohio, name and city only
{ "find": "customers",
  "filter": { "address.state": "OH" },
  "projection": { "company_name": 1, "address.city": 1 },
  "sort": { "company_name": 1 },
  "limit": 50 }

{ "aggregate": "orders", "pipeline": [
  { "$unwind": "$lines" },
  { "$group": { "_id": "$status", "lines": { "$sum": 1 } } } ] }
```

- [[key:run_statement]] runs the command under the cursor, [[key:run_script]] runs them all in order. `//` comments are allowed.
- Special values in extended JSON: `{ "$oid": "…" }`, `{ "$date": "2026-10-29T10:00:00Z" }`, `{ "$numberDecimal": "29.98" }`.
- In an `aggregate` the `cursor` can be left out.
- Syntax errors are underlined as you type.
- Autocomplete offers commands, keys, operators (`$match`, `$gt`…), collections after `"find":` and the collection's fields; after `"$`, field references (`$customer._id`).
- [[key:format]] rewrites each command over several lines; a command with comments stays as it is.
- [[key:cancel]] stops the cursor and asks the server to stop the operation.

## Read the results
Documents arrive in batches up to the connection's row limit or the cap in **Settings › Results**. Above the results, pick the view:
- **Documents**: the list on the left, the chosen document on the right as a tree, with each value's BSON type (`objectId`, `date`, `decimal`…). **Search** finds keys and values in the document; right-click copies a path or a value.
- **Table**: top-level fields become columns; subdocuments and arrays sit in a cell as JSON. Selection, copy, value panel and chart all work.

From the command palette: “Show documents as a tree” and “Show documents as a table”.

## Edit a document
Documents from a `find` without a `projection` can be edited.
1. Pick the document in the **Documents** view.
2. **Edit** (or double-click, or [[Enter]] on the row) opens the document as JSON.
3. Change the text and press **Save**. The document is replaced by `_id`, which does not change.

- **Duplicate** opens a copy without `_id`; **New document** opens an empty one. **Insert** adds it to the collection.
- **Delete** removes the document by `_id`.
- If someone else deleted the document meanwhile, NiceDB says so instead of creating a new one.

## Production and the write log
Every command goes through the same check as SQL statements ([Production safety](production)):
- `find`, `aggregate`, `count`, `distinct`, `explain` and the other read commands run even while writes are locked;
- `insert`, `update`, `delete`, `drop`, `createIndexes` and any unknown command need writes unlocked; an `aggregate` with `$out` or `$merge` is a write;
- `delete` or `update` on every document, `drop`, `dropDatabase`, `dropIndexes`, `collMod`, `renameCollection` ask for confirmation as risky.

Edits from the **Documents** view are logged as the equivalent command.

## Limitations
- There is no Data view, Schema or Tools for MongoDB.
- No graphical plan: write `{ "explain": { "find": … }, "verbosity": "executionStats" }` and read the document.
- No transactions from the editor, dry run, `:name` parameters or sorting from the column headers.
- No TLS client certificate, users or roles.
- A rare field may be missing from the inferred fields, which come from 100 documents.
- Through a tunnel or proxy the server is seen as `127.0.0.1`: with **Verify certificate** the certificate must also be valid for that name, otherwise use **Encrypted, unverified**.
- Without the privilege to kill operations, [[key:cancel]] stops the cursor but not an operation already running.
