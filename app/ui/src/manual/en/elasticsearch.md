# Elasticsearch and OpenSearch

Elasticsearch and OpenSearch keep JSON documents in indices. NiceDB talks to them as their console does: a request with its JSON body, or SQL.

## Connecting
1. **New connection**, then pick Elasticsearch under **Engine** (OpenSearch too).
2. **Host** and **Port** (9200). With TLS NiceDB uses HTTPS.
3. **Indices (patterns)**: empty shows every index; with `orders*, customers` the tree shows only those.
4. **User** and **Password** to sign in as a user; with an empty user, the **Password** holds the encoded API key (the `encoded` value the cluster returns when you create it); with both empty, a cluster without security. Password and key sit in the keychain.

## Exploring
Under the connection are **Indices**, **Aliases** and **Data streams**, with their document counts. Hovering an index shows its health and size; hovering an alias, the indices behind it. Opening an index shows the mapping's fields, with dotted paths (`customer.name`) and sub-fields (`customer.name.keyword`).

A click on an index opens a `match_all` search of 100 documents. **Show DDL** gives the `PUT` that makes the index again with settings, mapping and aliases.

## Writing
```
GET /orders/_search
{ "query": { "term": { "status": "open" } }, "size": 50 }

SELECT status, COUNT(*) FROM orders GROUP BY status;
```
- A request starts with the method and the path; the JSON body sits on the lines below. Lines starting with `#` or `//` are comments.
- A line starting with `SELECT` is SQL and goes to the cluster's SQL endpoint.
- Completion offers methods, indices and endpoints after the method, and in the body the Query DSL keys and the fields of the request's index.
- A JSON mistake is underlined while you type. **Format** writes bodies over several lines.
- `_cat/…` gets `format=json` by itself and arrives as a grid.

## Results
Search results are documents with `_id` and `_index` first, then the `_source` in its own order. A search with `"size": 0` and aggregations arrives as a grid: one row per innermost bucket, with the metrics beside it. Other replies arrive as one document.

In OpenSearch SQL names with dashes or reserved words go in backticks (`` `orders-2026` ``), in Elasticsearch SQL in double quotes; NiceDB picks by itself from the product. The quick snippets of the **Notes** panel only read (`topn`, `countby`, `dups`, `nulls`, `page`), and **Copy as** offers no `INSERT`, because the SQL of these engines does not write.

## Editing documents
Documents of a `_search` that does not filter the `_source` are edited by `_id`, in the index they come from (also when searching an alias). **Save** writes the document again if it is still there, **Insert** creates it, **Delete** removes it; after each write the index refreshes before answering. In production everything goes through the write gate and lands in the audit log as a request.

## Writes and production
`GET`, `HEAD` and the read-only `POST`s (`_search`, `_count`, `_msearch`, `_explain`, `_validate`, `_sql`…) read; the rest writes. Deleting an index, `_delete_by_query` or `_update_by_query` on every document, `_close` and cluster settings ask for confirmation as risky. [[key:cancel]] also cancels the running request on the server.

## Explain: the profile
**Explain** repeats the `_search` with `"profile": true` and draws it as a plan: one branch per shard, with the Lucene queries, the collectors and the aggregations, each with its time.

## Tools › Cluster
Health, version, nodes, active and unassigned shards; then the tables of nodes (memory, CPU, disk), indices and shards, with the state in colour.
