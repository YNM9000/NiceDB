import type { ServerInfo } from './bindings';
import { databaseOf, type Target } from './drivers';

const PRODUCTS: Record<Target['driver'], [string, string]> = {
  postgres: ['PostgreSQL', '16.10'],
  mysql: ['MySQL', '8.4.6'],
  sqlserver: ['SQL Server', '2022 (16.0.4135.4)'],
  cockroach: ['CockroachDB', '26.3.3'],
  sqlite: ['SQLite', '3.50.4'],
  duckdb: ['DuckDB', '1.4.1'],
  libsql: ['libSQL', '3.45.1'],
  mongodb: ['MongoDB', '7.0.25'],
  clickhouse: ['ClickHouse', '24.8.14.39'],
  redis: ['Redis', '7.4.12'],
  cassandra: ['ScyllaDB', '6.2.3'],
  dynamodb: ['DynamoDB Local', '2012-08-10'],
  elasticsearch: ['OpenSearch', '2.18.0'],
  firebird: ['Firebird', '5.0.3'],
  oracle: ['Oracle', '23.26.0.0.0'],
  db2: ['Db2', 'DB2 v12.1.2.0'],
  hana: ['SAP HANA', '2.00.079.00'],
  odbc: ['Informix', '14.10.FC11'],
};

export function mockServer(t: Target): ServerInfo {
  const [product, version] = PRODUCTS[t.driver];
  return { product, version, database: databaseOf(t) };
}
