# DynamoDB

DynamoDB keeps items in tables with a partition key and, sometimes, a sort key. NiceDB reads them with PartiQL or with API calls written as JSON, and shows them as documents.

## Connecting
1. **New connection**, then pick DynamoDB under **Engine**.
2. **Region**, such as `eu-west-1`.
3. **Endpoint**: empty uses the region's own; for DynamoDB Local write `http://127.0.0.1:8000`.
4. Credentials come from the **AWS profile** (read from `~/.aws/credentials`), or from **Access key ID** and **Secret access key**, with the secret in the keychain. Without either NiceDB uses the AWS environment variables.

## Exploring
Under the connection are the region's **Tables**, with their item counts. Opening one shows the key (partition and sort), the attributes found in the first 50 items with their type (`S`, `N`, `SS`…; when an attribute changes type, the share of each), and the global and local indexes.

Right click on a table:
- **Scan or Query…** opens the builder: pick Query or Scan, the table or an index, the partition key value, a condition on the sort key, filters, limit and order. Below you see the call it makes; **Run** opens it in a new tab, **Open in editor** leaves it for you to adjust;
- **Scan the table** writes a `Scan` call;
- **Query by key** writes a `Query` on the table's key, with placeholders to fill;
- **Query on index …** does the same for each index;
- **Show DDL** gives the `CreateTable` call that makes it again, empty.

## Writing
The editor takes two things, one after the other:
- PartiQL statements ending with `;`: `SELECT * FROM "Catalog" WHERE item = 'ART-104';`
- API calls as one JSON object named after the operation:

```json
{ "Query": { "TableName": "Catalog",
  "KeyConditionExpression": "#pk = :pk",
  "ExpressionAttributeNames": { "#pk": "item" },
  "ExpressionAttributeValues": { ":pk": { "S": "ART-104" } } } }
```

`Scan` and `Query` follow the pages up to the row limit; with `Limit` they read one only. **Format** writes the JSON calls over several lines.

In PartiQL table and attribute names go in double quotes and strings in single quotes; NiceDB always quotes the names it writes, because DynamoDB reserves hundreds of words (`status`, `name`, `size`…). PartiQL has no `LIMIT`: DynamoDB decides the pages. Of the quick snippets in the **Notes** panel, `ins` remains, writing an `INSERT INTO "table" VALUE {…}`.

## Results
Each item is a document, key attributes first. Numbers stay JSON numbers when they read back exactly, otherwise they become `{ "$numberDecimal": "…" }`; sets are `{ "$set": [...] }`; binaries `{ "$binary": … }`. So an edit puts every attribute back with its type. In the **Table**, **Copy as › INSERT** writes one `INSERT INTO … VALUE {…}` per item, with sets as `<<…>>`.

## Editing items
Items of a `SELECT *` or of a `Scan`/`Query` without projection are edited by key: **Save** writes the item only if it is still there, **Insert** only if the key is free, **Delete** only if the item exists. In production every write goes through the write gate and lands in the audit log as a `PutItem` or `DeleteItem` call.

## Writes and production
`SELECT` and the `Scan`, `Query`, `GetItem`, `BatchGetItem`, `Describe…` and `List…` calls read; the rest writes. `DeleteTable` and `UpdateTable` ask for confirmation as risky.

## Tools › Cluster
Region, endpoint, tables, items and size in all; for each table its status, items, size, capacity (on demand, or RCU and WCU), key and number of indexes. The item count is the one DynamoDB updates about every six hours.
