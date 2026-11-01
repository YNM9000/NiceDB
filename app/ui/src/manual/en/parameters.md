# Query parameters

A query can hold placeholders instead of values. When the statement under the cursor, or the selected text, contains some, the **Parameters** bar appears above the results with one field for each. In the editor the placeholders are underlined.

```sql
select number, total, due_date
from invoices
where customer_id = :customer
  and due_date >= ${from}
  and total > ?
```

## Write placeholders
| Form | Example | Note |
|---|---|---|
| `:name` | `where id = :id` | |
| `${name}` | `where day >= ${from}` | handy where `:` would be confusing |
| `?` | `where total > ?` | each one is a value of its own: in the bar it becomes `?1`, `?2`… in text order |

Plus the engines' own forms: `$1`, `$2`… on PostgreSQL, CockroachDB and DuckDB; `@name` on SQL Server (not `@@ROWCOUNT`, not `DECLARE` variables, not the arguments of `EXEC p @arg = …`); `@name` and `$name` on SQLite and libSQL. Inside strings, comments and `$$ … $$` blocks they are not placeholders.

`:id`, `${id}`, `@id` and `$id` are the same parameter and show up once. A name uses letters, digits and `_` and does not start with a digit.

These are not placeholders:
- text inside strings, quoted identifiers, comments and `$$ … $$` blocks;
- `::int` casts and `:=` assignments;
- a `:` glued to a word (`a:b`, `arr[1:n]`) and MySQL labels (`my_loop: loop`);
- on PostgreSQL and DuckDB, `?` used as a JSON operator (`data ? 'key'`);
- on SQLite, `?2`, which stays the engine's numbered placeholder.

## Run with parameters
1. Write the query with placeholders and press [[key:run_statement]].
2. If a parameter has no value yet, the query does not start: the bar highlights the new fields and puts the cursor in the first.
3. Type the values in the fields.
4. Press [[Enter]] or **Run** in the bar: exactly what you were running goes, statement, selection or script.

![The parameters bar filling :customer](anim:params-bar)

[[Esc]] in a field goes back to the editor. From the palette: "Go to the parameters" and "Clear the parameter values". **Run all** asks for the values of every statement in the script, with `?` numbered across the whole text.

## Types
Each field shows its type next to it; click to change it.

| Type | What it takes |
|---|---|
| **Text** | any text, empty too |
| **Number** | `42`, `-3.5`, `1e6` |
| **Date** | `2026-01-31`, with a time too |
| **Yes/No** | a **yes** / **no** switch |
| **NULL** | nothing: the value is `NULL` |
| **SQL name** | a table or column name, `schema.table` too |

Until you pick a type, the bar guesses it from what you type: `1042` becomes **Number**, `2026-01-01` **Date**. Picked by hand, it stays. A value that does not fit its type marks the field and stops the run: "The value of :id is not a valid number."

**SQL name** is for places a value cannot go, that is in place of a name: `select * from ${table}`. The field turns yellow and the bar keeps a warning. It takes only letters, digits, `_`, `$` and dots.

## Values used
The clock next to a field lists values used before: first **In this query**, then **In other queries** for the same name. Click to take a value back. NiceDB keeps up to 12 per name; when you reopen a query you used before, the fields start from the last values.

Values used stay on this computer and are not included in a `.nicedb` package.

## Saved scripts with parameters
**Save script** also keeps the values of the parameters in the text; the dialog lists them before saving. In the script list, parameter names show next to the tags. Opening or running the script starts the bar from those values; **Update from the editor** saves the current text and values. See [Notes, scripts and snippets](notes).

## Safety
Values are never pasted into the text as they are:
- text and dates go as real driver parameters on all eight relational engines; numbers, yes/no and NULL enter the text only after the type check;
- on PostgreSQL and CockroachDB, when the result has columns of a type NiceDB cannot read in binary exactly as the server writes it as text (`timestamptz`, `interval`, arrays, extension types), that statement runs with the values as literals in the dialect's quoting, which cannot break out of the string.

The **dry run**, the risky-statement confirmation and the production confirmation look at the SQL with the values in place: `delete from customers where id = :id` is not a DELETE without WHERE. History and the write log keep the final SQL. **Explain** uses the same bar.

## Limitations
- The **Date** type does not check that the date exists: the server rejects `2026-02-31`.
- The bar does not know the type of the column being compared: pick the type or let it be guessed.
- Values typed in the bar are saved with the workspace and come back with the tab.
