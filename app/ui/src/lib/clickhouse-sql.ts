import { SQLDialect } from '@codemirror/lang-sql';

export const CLICKHOUSE_KEYWORDS =
  'select from where prewhere group by order limit offset having with totals rollup cube union all distinct as on using join inner left right full cross outer semi anti asof any array global in not and or is null like ilike between case when then else end insert into values format settings create table view materialized dictionary database if exists engine partition primary key sample ttl to alter add drop modify column comment rename detach attach optimize final deduplicate kill query sync async system show describe desc explain plan pipeline estimate syntax ast use truncate exchange delete update lightweight mutation freeze unfreeze replace move fetch interval default materialize ephemeral alias codec projection index granularity type source lifetime layout range temporary function grant revoke role user quota profile policy watch live window over filter qualify';

export const CLICKHOUSE_TYPES =
  'UInt8 UInt16 UInt32 UInt64 UInt128 UInt256 Int8 Int16 Int32 Int64 Int128 Int256 Float32 Float64 Decimal Decimal32 Decimal64 Decimal128 Decimal256 Bool String FixedString UUID Date Date32 DateTime DateTime64 Enum8 Enum16 LowCardinality Nullable Array Tuple Map Nested JSON IPv4 IPv6 AggregateFunction SimpleAggregateFunction Nothing Interval Point Ring Polygon MultiPolygon';

export const CLICKHOUSE_FUNCTIONS =
  'count countIf sum sumIf avg avgIf min max any anyLast argMin argMax uniq uniqExact uniqCombined uniqHLL12 groupArray groupUniqArray groupArrayInsertAt topK quantile quantiles quantileExact quantileTDigest median stddevPop stddevSamp varPop corr sumMap minMap maxMap histogram retention windowFunnel sequenceMatch sequenceCount countDistinct ' +
  'if multiIf ifNull nullIf coalesce assumeNotNull toNullable isNull isNotNull ' +
  'toString toInt8 toInt16 toInt32 toInt64 toUInt8 toUInt16 toUInt32 toUInt64 toFloat32 toFloat64 toDecimal32 toDecimal64 toDecimal128 toDate toDate32 toDateTime toDateTime64 toUUID toTypeName CAST accurateCast toInt64OrNull toFloat64OrNull toDateOrNull parseDateTimeBestEffort parseDateTimeBestEffortOrNull reinterpretAsString ' +
  'now now64 today yesterday toYear toQuarter toMonth toDayOfMonth toDayOfWeek toHour toMinute toSecond toStartOfYear toStartOfQuarter toStartOfMonth toStartOfWeek toMonday toStartOfDay toStartOfHour toStartOfFiveMinutes toStartOfInterval toYYYYMM toYYYYMMDD dateDiff dateAdd dateSub addDays addMonths subtractDays formatDateTime toRelativeDayNum timeSlot ' +
  'length lengthUTF8 empty notEmpty lower upper lowerUTF8 upperUTF8 concat concatWithSeparator substring substringUTF8 trim trimBoth trimLeft trimRight leftPad rightPad replaceAll replaceOne replaceRegexpAll position positionCaseInsensitiveUTF8 positionUTF8 match extract extractAll like splitByChar splitByString arrayStringConcat format formatReadableSize formatReadableQuantity formatReadableTimeDelta base64Encode base64Decode hex unhex ' +
  'arrayJoin arrayMap arrayFilter arrayExists arrayAll arraySum arrayCount arraySort arrayReverseSort arrayDistinct arrayElement arrayEnumerate arrayZip arraySlice arrayConcat arrayReduce has hasAll hasAny indexOf range emptyArrayString tuple tupleElement map mapKeys mapValues ' +
  'JSONExtract JSONExtractString JSONExtractInt JSONExtractFloat JSONExtractBool JSONExtractRaw JSONHas JSON_VALUE JSON_QUERY visitParamExtractString ' +
  'cityHash64 sipHash64 xxHash64 murmurHash3_64 MD5 SHA256 rand rand64 generateUUIDv4 round floor ceil abs intDiv modulo greatest least exp log pow sqrt ' +
  'dictGet dictGetOrDefault dictHas currentDatabase currentUser version hostName uptime queryID getSetting numbers zeros generateRandom file url s3 remote cluster mysql postgresql input values system.numbers';

export const ClickHouseSql = SQLDialect.define({
  keywords: CLICKHOUSE_KEYWORDS.toLowerCase(),
  types: CLICKHOUSE_TYPES,
  builtin: CLICKHOUSE_FUNCTIONS,
  backslashEscapes: true,
  hashComments: true,
  doubleDollarQuotedStrings: true,
  identifierQuotes: '`"',
  caseInsensitiveIdentifiers: false,
});
