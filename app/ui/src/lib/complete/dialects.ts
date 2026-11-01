import type { Dialect } from '../bindings';
import { CLICKHOUSE_FUNCTIONS, CLICKHOUSE_TYPES } from '../clickhouse-sql';
import { isRelational, type RelationalDialect } from '../sqlname';

export type Engine = 'postgres' | 'cockroach' | 'mysql' | 'mariadb' | 'sqlite' | 'libsql' | 'mssql' | 'duckdb' | 'clickhouse' | 'firebird' | 'hana' | 'oracle' | 'db2' | 'ansi';

export function engineOf(dialect: Dialect, product = ''): Engine {
  const p = product.toLowerCase();
  if (dialect === 'postgres' && p.includes('cockroach')) return 'cockroach';
  if (dialect === 'mysql' && p.includes('mariadb')) return 'mariadb';
  if (dialect === 'sqlite' && (p.includes('libsql') || p.includes('turso'))) return 'libsql';
  if (dialect === 'ansi_backtick') return 'ansi';
  return isRelational(dialect) ? dialect : 'postgres';
}

const COMMON_FUNCTIONS = 'count sum avg min max coalesce nullif cast abs round floor ceil lower upper length trim ltrim rtrim replace substring concat row_number rank dense_rank ntile lag lead first_value last_value'.split(' ');

const FUNCTIONS: Record<Engine, string> = {
  postgres:
    'string_agg array_agg json_agg jsonb_agg json_build_object jsonb_build_object jsonb_set jsonb_array_elements jsonb_each to_json to_jsonb to_char to_date to_timestamp to_number date_trunc date_part extract age now current_date current_timestamp clock_timestamp make_date make_interval generate_series unnest array_length cardinality greatest least split_part regexp_replace regexp_matches regexp_split_to_table position strpos left right lpad rpad initcap format md5 gen_random_uuid pg_size_pretty pg_total_relation_size pg_relation_size pg_sleep pg_cancel_backend pg_terminate_backend percentile_cont percentile_disc mode bool_and bool_or every random trunc mod power sqrt',
  cockroach:
    'string_agg array_agg json_agg jsonb_agg json_build_object jsonb_build_object to_char date_trunc extract age now current_date current_timestamp generate_series unnest array_length greatest least split_part regexp_replace position strpos left right lpad rpad initcap md5 gen_random_uuid unique_rowid experimental_strftime percentile_cont percentile_disc bool_and bool_or random trunc mod power sqrt',
  mysql:
    'group_concat ifnull if isnull concat_ws date_format str_to_date datediff date_add date_sub timestampdiff timestampadd curdate curtime now sysdate unix_timestamp from_unixtime year month day dayofweek last_day json_extract json_unquote json_object json_array json_arrayagg json_objectagg json_contains json_length last_insert_id found_rows uuid md5 sha2 left right lpad rpad locate instr char_length format greatest least rand truncate mod power sqrt',
  mariadb:
    'group_concat ifnull if isnull concat_ws date_format str_to_date datediff date_add date_sub timestampdiff curdate curtime now sysdate unix_timestamp from_unixtime year month day last_day json_extract json_unquote json_object json_array json_arrayagg json_objectagg json_detailed json_query json_value last_insert_id found_rows uuid md5 sha2 left right lpad rpad locate instr char_length format greatest least rand truncate mod power sqrt',
  sqlite:
    'ifnull iif group_concat total typeof instr printf format strftime date time datetime julianday unixepoch json json_extract json_object json_array json_group_array json_group_object json_each json_tree random randomblob hex quote last_insert_rowid changes total_changes sqlite_version glob like likelihood zeroblob',
  libsql:
    'ifnull iif group_concat total typeof instr printf format strftime date time datetime julianday unixepoch json json_extract json_object json_array json_group_array json_each random randomblob hex quote last_insert_rowid changes sqlite_version vector vector32 vector_extract vector_distance_cos vector_top_k',
  mssql:
    'isnull iif getdate getutcdate sysdatetime sysutcdatetime datediff datediff_big dateadd datepart datename datefromparts eomonth format convert try_convert try_cast len datalength charindex patindex left right ltrim rtrim string_agg string_split concat_ws newid newsequentialid scope_identity object_id object_name db_name schema_name suser_sname row_number choose json_value json_query openjson isjson stuff replicate reverse square power sqrt',
  duckdb:
    'string_agg list list_agg array_agg list_value list_contains unnest range generate_series date_trunc date_part datediff date_diff date_add strftime strptime epoch epoch_ms make_date age now current_date today regexp_matches regexp_replace regexp_extract split_part starts_with ends_with contains read_csv read_csv_auto read_parquet read_json read_json_auto glob struct_pack struct_extract map arg_max arg_min median quantile_cont mode approx_count_distinct bool_and bool_or histogram typeof random greatest least',
  clickhouse: CLICKHOUSE_FUNCTIONS,
  firebird: 'first skip list iif decode char_length octet_length position overlay lpad rpad left right reverse hash dateadd datediff extract current_date current_time current_timestamp localtimestamp gen_id gen_uuid uuid_to_char char_to_uuid rdb$get_context rdb$set_context list',
  oracle:
    'nvl nvl2 decode to_char to_date to_timestamp to_number trunc add_months months_between last_day next_day sysdate systimestamp current_date extract numtodsinterval numtoyminterval instr substr regexp_like regexp_substr regexp_replace regexp_instr regexp_count listagg lpad rpad initcap translate chr ascii greatest least mod power sqrt sign rownum rowidtochar hextoraw rawtohex sys_guid sys_context user uid json_value json_query json_object json_arrayagg json_table xmlagg dbms_random.value dbms_lob.substr dbms_lob.getlength dbms_metadata.get_ddl dbms_output.put_line dbms_stats.gather_table_stats ratio_to_report percentile_cont median stddev variance',
  hana: 'ifnull nullif to_varchar to_nvarchar to_date to_timestamp to_decimal add_days add_months days_between seconds_between current_date current_timestamp now locate substr_after substr_before string_agg row_number series_generate_integer',
  db2: 'coalesce value nullif varchar char decimal digits days months_between add_months last_day current date current timestamp current schema listagg locate posstr substr translate trim rtrim ltrim to_char to_date timestamp_format varchar_format row_number rank dense_rank lag lead hex decfloat',
  ansi: '',
};

const cache = new Map<Engine, string[]>();

export function functionsOf(engine: Engine): string[] {
  let list = cache.get(engine);
  if (!list) {
    list = [...new Set([...COMMON_FUNCTIONS, ...FUNCTIONS[engine].split(' ')])];
    cache.set(engine, list);
  }
  return list;
}

const ANSI_TYPES = 'smallint integer bigint decimal numeric real double precision boolean char varchar character varying clob blob date time timestamp';

const TYPES: Record<RelationalDialect, string> = {
  postgres: 'integer bigint smallint numeric decimal real double precision boolean text varchar char date time timestamp timestamptz interval uuid json jsonb bytea inet serial bigserial',
  mysql: 'int bigint smallint tinyint decimal double float bit boolean char varchar text mediumtext longtext date datetime timestamp time year json blob binary varbinary enum',
  mssql: 'int bigint smallint tinyint bit decimal numeric money float real date datetime datetime2 datetimeoffset time char varchar nchar nvarchar text ntext uniqueidentifier varbinary xml',
  sqlite: 'integer real text blob numeric',
  duckdb: 'integer bigint smallint tinyint hugeint double float decimal boolean varchar text date time timestamp timestamptz interval uuid json blob list struct map',
  clickhouse: CLICKHOUSE_TYPES,
  firebird: 'smallint integer bigint int128 numeric decimal decfloat float double precision boolean char varchar blob date time timestamp',
  oracle: 'number integer float binary_float binary_double varchar2 nvarchar2 char nchar clob nclob blob raw date timestamp interval boolean json rowid urowid long xmltype vector',
  hana: 'tinyint smallint integer bigint decimal smalldecimal real double boolean varchar nvarchar alphanum shorttext varbinary blob clob nclob text date time seconddate timestamp',
  db2: 'smallint integer bigint decimal decfloat real double boolean char varchar clob graphic vargraphic dbclob binary varbinary blob date time timestamp xml',
  ansi: ANSI_TYPES,
  ansi_backtick: ANSI_TYPES,
};

export const typesOf = (d: Dialect) => TYPES[isRelational(d) ? d : 'postgres'].split(' ').map((t) => t.replace(' ', ' '));

export const STATEMENT_START = ['select', 'insert into', 'update', 'delete from', 'with', 'create table', 'create view', 'create index', 'alter table', 'drop table', 'truncate table', 'explain', 'begin', 'commit', 'rollback'];

export const AFTER_RELATION = ['where', 'join', 'left join', 'inner join', 'right join', 'full join', 'cross join', 'on', 'as', 'group by', 'order by', 'having', 'limit', 'union', 'set'];

export const EXPRESSION = ['and', 'or', 'not', 'in', 'is null', 'is not null', 'like', 'between', 'exists', 'case', 'when', 'then', 'else', 'end', 'distinct', 'as', 'from', 'where', 'group by', 'order by', 'asc', 'desc', 'null', 'true', 'false'];

export const CLAUSE_WORDS = new Set(
  'select from where group order by having limit offset fetch join inner left right full outer cross natural on using union intersect except minus window returning values set into as and or not with lateral apply when then else end case is in like ilike between exists for top distinct all only qualify pivot unpivot tablesample go option'.split(' '),
);

const RESERVED = new Set(
  'add all alter analyze and any array as asc authorization backup between both by cascade case cast check collate column commit constraint create cross current current_date current_time current_timestamp current_user database default delete desc distinct do drop else end except exec exists false fetch file for foreign from full function grant group having if in index inner insert intersect interval into is join key keys leading left like limit lock natural not null offset on only or order outer over percent plan primary procedure range read references rename replace right row rows schema select session_user set some table then to top trailing transaction trigger true union unique update user using values view when where window with'.split(' '),
);

export const NOT_COLUMNS = new Set([
  ...RESERVED,
  ...CLAUSE_WORDS,
  ...'interval date time timestamp year month day hour minute second epoch week quarter dow doy zone at local localtime localtimestamp current_schema current_catalog current_role sysdate systimestamp unknown nulls first last asc desc ilike similar escape collate filter over partition rows range preceding following unbounded current row ties array row any some cascade restrict if replace ignore duplicate conflict nothing do excluded recursive materialized global temporary temp separator binary signed unsigned char integer int varchar text numeric decimal boolean bool bigint smallint float real double precision varying nvarchar nchar bit uniqueidentifier datetime datetime2 money json jsonb uuid bytea blob output inserted deleted nolock holdlock readpast rowlock updlock tablock top percent identity rowcount trancount error identitycol rowguidcol'.split(' '),
]);

const ORACLE_RESERVED = new Set(
  'access audit char cluster comment compress connect date decimal exclusive float identified immediate increment initial integer level long maxextents minus mlslabel mode modify noaudit nocompress nowait number of offline online option pctfree prior privileges public raw resource rowid rownum session share size smallint start successful synonym sysdate uid validate varchar varchar2 whenever'.split(' '),
);

const CQL_RESERVED = new Set('allow apply authorize batch begin columnfamily describe entries execute infinity keyspace materialized mbean mbeans modify nan norecursive of revoke token truncate unlogged use'.split(' '));

export function needsQuote(name: string, dialect: Dialect): boolean {
  if (!name) return true;
  if (dialect === 'partiql') return true;
  const plain =
    dialect === 'postgres' || dialect === 'duckdb'
      ? /^[a-z_][a-z0-9_$]*$/
      : dialect === 'cql'
        ? /^[a-z_][a-z0-9_]*$/
        : dialect === 'oracle'
          ? /^[A-Z][A-Z0-9_$#]*$/
        : dialect === 'firebird' || dialect === 'hana' || dialect === 'db2'
          ? /^[A-Z_][A-Z0-9_$]*$/
          : dialect === 'ansi' || dialect === 'ansi_backtick'
            ? /^([a-z_][a-z0-9_]*|[A-Z_][A-Z0-9_]*)$/
          : dialect === 'opensearch' || dialect === 'elasticsearch'
            ? /^[A-Za-z_][A-Za-z0-9_]*$/
            : /^[A-Za-z_][A-Za-z0-9_$]*$/;
  const word = name.toLowerCase();
  return !plain.test(name) || RESERVED.has(word) || (dialect === 'cql' && CQL_RESERVED.has(word)) || (dialect === 'oracle' && ORACLE_RESERVED.has(word));
}

export const DEFAULT_NAMESPACES = new Set(['public', 'main', 'dbo']);
