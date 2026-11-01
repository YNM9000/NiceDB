use nd_driver::{Connection, Row};
use serde::Serialize;

use super::{Engine, ToolFault};
use crate::guard::{Access, collect};
use crate::{Core, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum RoleAttribute {
    Login,
    Superuser,
    CreateDb,
    CreateRole,
    Locked,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Principal {
    pub name: String,
    pub host: Option<String>,
    pub attributes: Vec<RoleAttribute>,
    pub member_of: Vec<String>,
}

fn attributes(flags: [(RoleAttribute, bool); 5]) -> Vec<RoleAttribute> {
    flags
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(a, _)| a)
        .collect()
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Grant {
    pub grantee: String,
    pub kind: String,
    pub schema: Option<String>,
    pub object: Option<String>,
    pub privilege: String,
    pub grantable: bool,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Principals {
    pub principals: Vec<Principal>,
    pub grants: Vec<Grant>,
    pub partial: bool,
    pub quotas: Vec<Quota>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct Quota {
    pub name: String,
    pub all: bool,
    pub applies_to: Vec<String>,
    pub except: Vec<String>,
    pub limits: Vec<QuotaLimit>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct QuotaLimit {
    pub seconds: f64,
    pub caps: Vec<QuotaCap>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct QuotaCap {
    pub what: String,
    pub max: f64,
}

const PG_ROLES: &str = "SELECT r.rolname, r.rolcanlogin::text, r.rolsuper::text, r.rolcreatedb::text, \
r.rolcreaterole::text, (SELECT string_agg(g.rolname, ',' ORDER BY g.rolname) FROM pg_auth_members m \
JOIN pg_roles g ON g.oid = m.roleid WHERE m.member = r.oid), \
(r.rolvaliduntil IS NOT NULL AND r.rolvaliduntil < now())::text \
FROM pg_roles r WHERE r.rolname NOT LIKE 'pg\\_%' ORDER BY r.rolname";

const PG_GRANTS: &str = "SELECT * FROM (\
SELECT coalesce(g.rolname, 'PUBLIC') AS who, CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'view' WHEN 'S' THEN 'sequence' ELSE 'table' END AS kind, \
n.nspname AS sch, c.relname AS obj, a.privilege_type AS priv, a.is_grantable::text AS opt \
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace CROSS JOIN LATERAL aclexplode(c.relacl) a \
LEFT JOIN pg_roles g ON g.oid = a.grantee \
WHERE c.relkind IN ('r', 'p', 'v', 'm', 'S') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg\\_%' \
UNION ALL SELECT pg_get_userbyid(c.relowner), CASE c.relkind WHEN 'v' THEN 'view' WHEN 'm' THEN 'view' WHEN 'S' THEN 'sequence' ELSE 'table' END, \
n.nspname, c.relname, 'OWNER', 'true' FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
WHERE c.relkind IN ('r', 'p', 'v', 'm', 'S') AND n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg\\_%' \
UNION ALL SELECT coalesce(g.rolname, 'PUBLIC'), 'schema', n.nspname, NULL, a.privilege_type, a.is_grantable::text \
FROM pg_namespace n CROSS JOIN LATERAL aclexplode(n.nspacl) a LEFT JOIN pg_roles g ON g.oid = a.grantee \
WHERE n.nspname NOT IN ('pg_catalog', 'information_schema') AND n.nspname NOT LIKE 'pg\\_%' \
UNION ALL SELECT coalesce(g.rolname, 'PUBLIC'), 'database', NULL, d.datname, a.privilege_type, a.is_grantable::text \
FROM pg_database d CROSS JOIN LATERAL aclexplode(d.datacl) a LEFT JOIN pg_roles g ON g.oid = a.grantee \
WHERE d.datname = current_database()) x ORDER BY who, kind, sch, obj, priv LIMIT 5000";

const MY_ACCOUNTS: &str = "SELECT User, Host, 'true', Super_priv, Create_priv, Create_user_priv FROM mysql.user ORDER BY User, Host";

const MY_LOCKED: &str = "SELECT User, Host, account_locked FROM mysql.user";

const MY_ROLES: &str = "SELECT User, Host FROM mysql.user WHERE account_locked = 'Y' AND authentication_string = '' \
UNION SELECT FROM_USER, FROM_HOST FROM mysql.role_edges";

const MY_EDGES: &str = "SELECT TO_USER, TO_HOST, FROM_USER FROM mysql.role_edges";

const MARIA_ROLES: &str = "SELECT User, Host FROM mysql.user WHERE is_role = 'Y'";

const MARIA_EDGES: &str = "SELECT User, Host, Role FROM mysql.roles_mapping";

const MY_GRANTS: &str = "SELECT GRANTEE, 'global', NULL, NULL, PRIVILEGE_TYPE, IS_GRANTABLE FROM information_schema.USER_PRIVILEGES \
WHERE PRIVILEGE_TYPE <> 'USAGE' \
UNION ALL SELECT GRANTEE, 'schema', TABLE_SCHEMA, NULL, PRIVILEGE_TYPE, IS_GRANTABLE FROM information_schema.SCHEMA_PRIVILEGES \
UNION ALL SELECT GRANTEE, 'table', TABLE_SCHEMA, TABLE_NAME, PRIVILEGE_TYPE, IS_GRANTABLE FROM information_schema.TABLE_PRIVILEGES \
ORDER BY 1, 2, 3, 4, 5 LIMIT 5000";

const MS_PRINCIPALS: &str = "SELECT dp.name, CASE WHEN dp.type IN ('S', 'U', 'G', 'E', 'X') THEN 1 ELSE 0 END, \
CASE WHEN dp.name = 'dbo' OR IS_ROLEMEMBER('db_owner', dp.name) = 1 OR IS_SRVROLEMEMBER('sysadmin', sp.name) = 1 THEN 1 ELSE 0 END, \
CASE WHEN IS_SRVROLEMEMBER('dbcreator', sp.name) = 1 OR IS_SRVROLEMEMBER('sysadmin', sp.name) = 1 THEN 1 ELSE 0 END, \
CASE WHEN IS_SRVROLEMEMBER('securityadmin', sp.name) = 1 OR IS_SRVROLEMEMBER('sysadmin', sp.name) = 1 OR IS_ROLEMEMBER('db_securityadmin', dp.name) = 1 THEN 1 ELSE 0 END, \
(SELECT STRING_AGG(r.name, ',') WITHIN GROUP (ORDER BY r.name) FROM sys.database_role_members m \
JOIN sys.database_principals r ON r.principal_id = m.role_principal_id WHERE m.member_principal_id = dp.principal_id), \
CASE WHEN sp.is_disabled = 1 THEN 1 ELSE 0 END \
FROM sys.database_principals dp LEFT JOIN sys.server_principals sp ON sp.sid = dp.sid \
WHERE dp.type IN ('S', 'U', 'G', 'R', 'E', 'X') AND dp.is_fixed_role = 0 \
AND dp.name NOT IN ('public', 'guest', 'INFORMATION_SCHEMA', 'sys') ORDER BY dp.name";

const MS_GRANTS: &str = "SELECT * FROM (\
SELECT gp.name AS who, CASE p.class WHEN 0 THEN 'database' WHEN 3 THEN 'schema' WHEN 1 THEN CASE WHEN o.type = 'V' THEN 'view' ELSE 'table' END ELSE 'table' END AS kind, \
CASE p.class WHEN 3 THEN sch.name WHEN 1 THEN os.name END AS sch, CASE p.class WHEN 1 THEN o.name WHEN 0 THEN DB_NAME() END AS obj, \
p.permission_name AS priv, CASE WHEN p.state = 'W' THEN 1 ELSE 0 END AS opt \
FROM sys.database_permissions p JOIN sys.database_principals gp ON gp.principal_id = p.grantee_principal_id \
LEFT JOIN sys.objects o ON p.class = 1 AND o.object_id = p.major_id \
LEFT JOIN sys.schemas os ON os.schema_id = o.schema_id \
LEFT JOIN sys.schemas sch ON p.class = 3 AND sch.schema_id = p.major_id \
WHERE p.state IN ('G', 'W') AND p.class IN (0, 1, 3) AND p.minor_id = 0 AND (p.class <> 1 OR o.is_ms_shipped = 0) \
UNION ALL SELECT own.name, 'schema', s.name, NULL, 'OWNER', 1 FROM sys.schemas s \
JOIN sys.database_principals own ON own.principal_id = s.principal_id \
WHERE s.schema_id < 16384 AND s.name NOT IN ('sys', 'INFORMATION_SCHEMA', 'guest')) x ORDER BY who, kind, sch, obj, priv";

const CRDB_ROLES: &str =
    "SELECT username, options::STRING, member_of::STRING FROM [SHOW ROLES] ORDER BY username";

const CRDB_GRANTS: &str = "SELECT * FROM (\
SELECT grantee AS who, object_type AS kind, schema_name AS sch, CASE WHEN object_type = 'database' THEN database_name ELSE object_name END AS obj, \
privilege_type AS priv, is_grantable::STRING AS opt FROM [SHOW GRANTS] \
WHERE coalesce(schema_name, '') NOT IN ('crdb_internal', 'information_schema', 'pg_catalog', 'pg_extension') AND object_type IN ('database', 'schema', 'table', 'view', 'sequence') \
UNION ALL SELECT owner, type, schema_name, table_name, 'OWNER', 'true' FROM [SHOW TABLES] \
WHERE schema_name NOT IN ('crdb_internal', 'information_schema', 'pg_catalog', 'pg_extension')) x ORDER BY who, kind, sch, obj, priv LIMIT 5000";

const CH_USERS: &str = "SELECT name, toString(auth_type) FROM system.users ORDER BY name";
const CH_ROLES: &str = "SELECT name FROM system.roles ORDER BY name";
const CH_ROLE_GRANTS: &str =
    "SELECT ifNull(user_name, role_name), granted_role_name FROM system.role_grants ORDER BY 1, 2";
const CH_GRANTS: &str = "SELECT ifNull(user_name, role_name), access_type, database, table, column, grant_option \
FROM system.grants WHERE is_partial_revoke = 0 ORDER BY 1, 3, 4, 2 LIMIT 5000";
const CH_QUOTAS: &str = "SELECT name, apply_to_all, arrayStringConcat(apply_to_list, ','), arrayStringConcat(apply_to_except, ',') FROM system.quotas ORDER BY name";
const CH_QUOTA_CAPS: [&str; 10] = [
    "max_queries",
    "max_query_selects",
    "max_query_inserts",
    "max_errors",
    "max_result_rows",
    "max_result_bytes",
    "max_read_rows",
    "max_read_bytes",
    "max_execution_time",
    "max_failed_sequential_authentications",
];

fn ch_quota_limits() -> String {
    format!(
        "SELECT quota_name, duration, {} FROM system.quota_limits ORDER BY quota_name, duration",
        CH_QUOTA_CAPS[..9].join(", ")
    )
}

const ORA_PRINCIPALS: &str = "SELECT u.username, 'true', \
CASE WHEN EXISTS (SELECT 1 FROM dba_role_privs r WHERE r.grantee = u.username AND r.granted_role = 'DBA') THEN 'true' ELSE 'false' END, \
CASE WHEN EXISTS (SELECT 1 FROM dba_sys_privs p WHERE p.grantee = u.username AND p.privilege = 'CREATE USER') THEN 'true' ELSE 'false' END, \
CASE WHEN EXISTS (SELECT 1 FROM dba_sys_privs p WHERE p.grantee = u.username AND p.privilege = 'CREATE ROLE') THEN 'true' ELSE 'false' END, \
(SELECT LISTAGG(r.granted_role, ',') WITHIN GROUP (ORDER BY r.granted_role) FROM dba_role_privs r WHERE r.grantee = u.username), \
CASE WHEN u.account_status LIKE '%LOCKED%' OR u.account_status LIKE '%EXPIRED%' THEN 'true' ELSE 'false' END \
FROM dba_users u WHERE u.oracle_maintained = 'N' OR u.username IN ('SYS', 'SYSTEM') \
UNION ALL SELECT r.role, 'false', 'false', 'false', 'false', \
(SELECT LISTAGG(p.granted_role, ',') WITHIN GROUP (ORDER BY p.granted_role) FROM dba_role_privs p WHERE p.grantee = r.role), 'false' \
FROM dba_roles r WHERE r.oracle_maintained = 'N' ORDER BY 1";

const ORA_PRINCIPALS_SAFE: &str = "SELECT u.username, 'true', 'false', 'false', 'false', \
CASE WHEN u.username = USER THEN (SELECT LISTAGG(granted_role, ',') WITHIN GROUP (ORDER BY granted_role) FROM user_role_privs) END, 'false' \
FROM all_users u WHERE u.oracle_maintained = 'N' OR u.username = USER ORDER BY 1";

const ORA_GRANTS: &str = "SELECT * FROM (\
SELECT p.grantee, CASE p.type WHEN 'VIEW' THEN 'view' WHEN 'SEQUENCE' THEN 'sequence' WHEN 'TABLE' THEN 'table' ELSE LOWER(p.type) END, \
p.owner, p.table_name, p.privilege, p.grantable FROM dba_tab_privs p JOIN dba_users o ON o.username = p.owner AND o.oracle_maintained = 'N' \
UNION ALL SELECT p.grantee, 'global', NULL, NULL, p.privilege, p.admin_option FROM dba_sys_privs p \
WHERE p.grantee IN (SELECT username FROM dba_users WHERE oracle_maintained = 'N' UNION SELECT role FROM dba_roles WHERE oracle_maintained = 'N') \
) ORDER BY 1, 2, 3, 4, 5 FETCH FIRST 5000 ROWS ONLY";

const ORA_GRANTS_SAFE: &str = "SELECT grantee, CASE type WHEN 'VIEW' THEN 'view' WHEN 'SEQUENCE' THEN 'sequence' WHEN 'TABLE' THEN 'table' ELSE LOWER(type) END, \
table_schema, table_name, privilege, grantable FROM all_tab_privs \
WHERE table_schema IN (SELECT username FROM all_users WHERE oracle_maintained = 'N') \
UNION ALL SELECT USER, 'global', NULL, NULL, privilege, admin_option FROM user_sys_privs ORDER BY 1, 2, 3, 4, 5";

fn cell(r: &Row, i: usize) -> Option<String> {
    r.get(i).cloned().flatten()
}

fn yes(r: &Row, i: usize) -> bool {
    matches!(
        cell(r, i).as_deref(),
        Some("true" | "t" | "Y" | "YES" | "1")
    )
}

fn names(list: Option<String>) -> Vec<String> {
    list.map(|s| s.split(',').map(str::to_owned).collect())
        .unwrap_or_default()
}

fn cockroach_role(r: &Row) -> Principal {
    let name = cell(r, 0).unwrap_or_default();
    let options = cell(r, 1).unwrap_or_default().to_ascii_uppercase();
    let has = |word: &str| {
        options
            .trim_matches(['{', '}'])
            .split(',')
            .any(|o| o.trim() == word)
    };
    let roles: Vec<String> = names(cell(r, 2).map(|s| s.trim_matches(['{', '}']).to_owned()))
        .into_iter()
        .filter(|g| !g.is_empty())
        .collect();
    let admin = name == "admin" || roles.iter().any(|g| g == "admin");
    Principal {
        host: None,
        attributes: attributes([
            (RoleAttribute::Login, !has("NOLOGIN")),
            (RoleAttribute::Superuser, admin),
            (RoleAttribute::CreateDb, has("CREATEDB") || admin),
            (RoleAttribute::CreateRole, has("CREATEROLE") || admin),
            (RoleAttribute::Locked, false),
        ]),
        member_of: roles,
        name,
    }
}

fn account(grantee: &str) -> String {
    grantee.replace('\'', "")
}

impl Core {
    pub async fn clickhouse_access(&self, session_id: &str) -> Result<super::ClickhouseAccess> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        if Engine::of(conn) != Engine::Clickhouse {
            return Err(ToolFault::Unsupported.into());
        }
        Ok(super::clickhouse::access(conn).await)
    }

    pub async fn principals(&self, session_id: &str) -> Result<Principals> {
        let session = self.live(session_id).await?;
        self.authorize(&session, Access::Read)?;
        let side = self.side(&session).await?;
        let conn = side.conn()?;
        match Engine::of(conn) {
            Engine::Postgres => {
                let roles = collect(conn, PG_ROLES, None).await;
                let grants = collect(conn, PG_GRANTS, None).await;
                let (roles, _) = side.check(roles)?;
                let (grants, _) = side.check(grants)?;
                Ok(Principals {
                    principals: roles
                        .iter()
                        .map(|r| Principal {
                            name: cell(r, 0).unwrap_or_default(),
                            host: None,
                            attributes: attributes([
                                (RoleAttribute::Login, yes(r, 1)),
                                (RoleAttribute::Superuser, yes(r, 2)),
                                (RoleAttribute::CreateDb, yes(r, 3)),
                                (RoleAttribute::CreateRole, yes(r, 4)),
                                (RoleAttribute::Locked, yes(r, 6)),
                            ]),
                            member_of: names(cell(r, 5)),
                        })
                        .collect(),
                    grants: grants.iter().map(|r| grant_of(r, false)).collect(),
                    partial: false,
                    quotas: Vec::new(),
                })
            }
            Engine::Cockroach => {
                let roles = collect(conn, CRDB_ROLES, None).await;
                let grants = collect(conn, CRDB_GRANTS, None).await;
                let (roles, _) = side.check(roles)?;
                let (grants, _) = side.check(grants)?;
                Ok(Principals {
                    principals: roles.iter().map(cockroach_role).collect(),
                    grants: grants.iter().map(|r| grant_of(r, false)).collect(),
                    partial: false,
                    quotas: Vec::new(),
                })
            }
            Engine::Mssql => {
                let found = collect(conn, MS_PRINCIPALS, None).await;
                let grants = collect(conn, MS_GRANTS, None).await;
                let (found, _) = side.check(found)?;
                let (grants, _) = side.check(grants)?;
                Ok(Principals {
                    principals: found
                        .iter()
                        .map(|r| Principal {
                            name: cell(r, 0).unwrap_or_default(),
                            host: None,
                            attributes: attributes([
                                (RoleAttribute::Login, yes(r, 1)),
                                (RoleAttribute::Superuser, yes(r, 2)),
                                (RoleAttribute::CreateDb, yes(r, 3)),
                                (RoleAttribute::CreateRole, yes(r, 4)),
                                (RoleAttribute::Locked, yes(r, 6)),
                            ]),
                            member_of: names(cell(r, 5)),
                        })
                        .collect(),
                    grants: grants.iter().map(|r| grant_of(r, false)).collect(),
                    partial: false,
                    quotas: Vec::new(),
                })
            }
            e @ (Engine::Mysql | Engine::Mariadb) => {
                let grants = collect(conn, MY_GRANTS, None).await;
                let (grants, _) = side.check(grants)?;
                let grants: Vec<Grant> = grants.iter().map(|r| grant_of(r, true)).collect();
                let maria = matches!(e, Engine::Mariadb);
                let (principals, partial) = mysql_accounts(conn, &grants, maria).await;
                Ok(Principals {
                    principals,
                    grants,
                    partial,
                    quotas: Vec::new(),
                })
            }
            Engine::Clickhouse => side.check(clickhouse_read(conn).await),
            Engine::Oracle => side.check(oracle_read(conn).await),
            Engine::Firebird => side.check(listed(conn, FB_PRINCIPALS, FB_GRANTS).await),
            Engine::Db2 => side.check(listed(conn, DB2_PRINCIPALS, DB2_GRANTS).await),
            Engine::Hana => side.check(hana_read(conn).await),
            Engine::Mongo => {
                let database = session.conn.server().database.clone();
                let found = super::mongo::principals(conn, &database).await;
                side.check(found)
            }
            Engine::Sqlite | Engine::Libsql | Engine::Duckdb | Engine::Odbc => {
                Err(ToolFault::Unsupported.into())
            }
        }
    }
}

const FB_PRINCIPALS: &str = "SELECT TRIM(u.SEC$USER_NAME), 'true', CASE WHEN u.SEC$ADMIN THEN 'true' ELSE 'false' END, \
'false', 'false', (SELECT LIST(TRIM(p.RDB$RELATION_NAME), ',') FROM RDB$USER_PRIVILEGES p \
  WHERE p.RDB$USER = u.SEC$USER_NAME AND p.RDB$PRIVILEGE = 'M'), \
CASE WHEN u.SEC$ACTIVE THEN 'false' ELSE 'true' END FROM SEC$USERS u \
UNION ALL SELECT TRIM(r.RDB$ROLE_NAME), 'false', 'false', 'false', 'false', \
(SELECT LIST(TRIM(p.RDB$RELATION_NAME), ',') FROM RDB$USER_PRIVILEGES p \
  WHERE p.RDB$USER = r.RDB$ROLE_NAME AND p.RDB$PRIVILEGE = 'M'), 'false' \
FROM RDB$ROLES r WHERE COALESCE(r.RDB$SYSTEM_FLAG, 0) = 0 ORDER BY 1";

const FB_GRANTS: &str = "SELECT TRIM(p.RDB$USER), CASE p.RDB$OBJECT_TYPE WHEN 0 THEN \
  CASE WHEN EXISTS (SELECT 1 FROM RDB$RELATIONS r WHERE r.RDB$RELATION_NAME = p.RDB$RELATION_NAME AND r.RDB$VIEW_BLR IS NOT NULL) \
  THEN 'view' ELSE 'table' END WHEN 5 THEN 'routine' WHEN 14 THEN 'sequence' WHEN 13 THEN 'role' ELSE 'table' END, \
CAST(NULL AS VARCHAR(8)), TRIM(p.RDB$RELATION_NAME), \
CASE TRIM(p.RDB$PRIVILEGE) WHEN 'S' THEN 'SELECT' WHEN 'I' THEN 'INSERT' WHEN 'U' THEN 'UPDATE' WHEN 'D' THEN 'DELETE' \
  WHEN 'R' THEN 'REFERENCES' WHEN 'X' THEN 'EXECUTE' WHEN 'G' THEN 'USAGE' ELSE TRIM(p.RDB$PRIVILEGE) END, \
CASE WHEN p.RDB$GRANT_OPTION = 1 THEN 'true' ELSE 'false' END \
FROM RDB$USER_PRIVILEGES p WHERE p.RDB$PRIVILEGE <> 'M' AND p.RDB$USER_TYPE IN (8, 13) \
AND p.RDB$RELATION_NAME NOT STARTING WITH 'RDB$' AND p.RDB$RELATION_NAME NOT STARTING WITH 'MON$' \
AND p.RDB$RELATION_NAME NOT STARTING WITH 'SEC$' ORDER BY 1, 2, 4, 5 ROWS 5000";

const DB2_PRINCIPALS: &str = "SELECT RTRIM(d.GRANTEE), CASE WHEN d.CONNECTAUTH = 'Y' THEN 'true' ELSE 'false' END, \
CASE WHEN d.DBADMAUTH = 'Y' THEN 'true' ELSE 'false' END, CASE WHEN d.CREATETABAUTH = 'Y' THEN 'true' ELSE 'false' END, \
CASE WHEN d.SECURITYADMAUTH = 'Y' THEN 'true' ELSE 'false' END, \
(SELECT LISTAGG(RTRIM(r.ROLENAME), ',') FROM SYSCAT.ROLEAUTH r WHERE r.GRANTEE = d.GRANTEE AND r.GRANTEETYPE = 'U'), 'false' \
FROM SYSCAT.DBAUTH d WHERE d.GRANTEETYPE = 'U' \
UNION ALL SELECT RTRIM(x.ROLENAME), 'false', 'false', 'false', 'false', \
(SELECT LISTAGG(RTRIM(r.ROLENAME), ',') FROM SYSCAT.ROLEAUTH r WHERE r.GRANTEE = x.ROLENAME AND r.GRANTEETYPE = 'R'), 'false' \
FROM SYSCAT.ROLES x WHERE x.ROLENAME NOT LIKE 'SYS%' ORDER BY 1";

const DB2_GRANTS: &str = "SELECT * FROM (\
SELECT RTRIM(GRANTEE) w, 'table' k, RTRIM(TABSCHEMA) s, RTRIM(TABNAME) o, 'SELECT' p, CASE WHEN SELECTAUTH = 'G' THEN 'true' ELSE 'false' END g \
FROM SYSCAT.TABAUTH WHERE SELECTAUTH IN ('Y', 'G') AND TABSCHEMA NOT LIKE 'SYS%' \
UNION ALL SELECT RTRIM(GRANTEE), 'table', RTRIM(TABSCHEMA), RTRIM(TABNAME), 'INSERT', CASE WHEN INSERTAUTH = 'G' THEN 'true' ELSE 'false' END \
FROM SYSCAT.TABAUTH WHERE INSERTAUTH IN ('Y', 'G') AND TABSCHEMA NOT LIKE 'SYS%' \
UNION ALL SELECT RTRIM(GRANTEE), 'table', RTRIM(TABSCHEMA), RTRIM(TABNAME), 'UPDATE', CASE WHEN UPDATEAUTH = 'G' THEN 'true' ELSE 'false' END \
FROM SYSCAT.TABAUTH WHERE UPDATEAUTH IN ('Y', 'G') AND TABSCHEMA NOT LIKE 'SYS%' \
UNION ALL SELECT RTRIM(GRANTEE), 'table', RTRIM(TABSCHEMA), RTRIM(TABNAME), 'DELETE', CASE WHEN DELETEAUTH = 'G' THEN 'true' ELSE 'false' END \
FROM SYSCAT.TABAUTH WHERE DELETEAUTH IN ('Y', 'G') AND TABSCHEMA NOT LIKE 'SYS%' \
UNION ALL SELECT RTRIM(GRANTEE), 'table', RTRIM(TABSCHEMA), RTRIM(TABNAME), 'CONTROL', 'false' \
FROM SYSCAT.TABAUTH WHERE CONTROLAUTH = 'Y' AND TABSCHEMA NOT LIKE 'SYS%' \
) x ORDER BY 1, 2, 3, 4, 5 FETCH FIRST 5000 ROWS ONLY";

const HANA_PRINCIPALS: &str = "SELECT u.USER_NAME, 'true', CASE WHEN u.USER_NAME = 'SYSTEM' THEN 'true' ELSE 'false' END, \
'false', 'false', (SELECT STRING_AGG(g.ROLE_NAME, ',') FROM SYS.GRANTED_ROLES g WHERE g.GRANTEE = u.USER_NAME), \
CASE WHEN u.USER_DEACTIVATED = 'TRUE' THEN 'true' ELSE 'false' END \
FROM SYS.USERS u WHERE u.USER_NAME NOT LIKE '\\_SYS%' ESCAPE '\\' AND u.USER_NAME <> 'SYS' \
UNION ALL SELECT r.ROLE_NAME, 'false', 'false', 'false', 'false', \
(SELECT STRING_AGG(g.ROLE_NAME, ',') FROM SYS.GRANTED_ROLES g WHERE g.GRANTEE = r.ROLE_NAME), 'false' \
FROM SYS.ROLES r ORDER BY 1";

const HANA_GRANTS: &str = "SELECT GRANTEE, CASE OBJECT_TYPE WHEN 'TABLE' THEN 'table' WHEN 'VIEW' THEN 'view' \
WHEN 'SCHEMA' THEN 'schema' WHEN 'SEQUENCE' THEN 'sequence' WHEN 'SYSTEMPRIVILEGE' THEN 'global' ELSE LOWER(OBJECT_TYPE) END, \
SCHEMA_NAME, OBJECT_NAME, PRIVILEGE, LOWER(IS_GRANTABLE) FROM SYS.GRANTED_PRIVILEGES \
WHERE GRANTEE NOT LIKE '\\_SYS%' ESCAPE '\\' AND GRANTEE <> 'SYS' ORDER BY 1, 2, 3, 4, 5 LIMIT 5000";

async fn hana_read(
    conn: &dyn Connection,
) -> std::result::Result<Principals, nd_driver::DriverError> {
    let mut found = listed(conn, HANA_PRINCIPALS, HANA_GRANTS).await?;
    found.partial = !super::activity::hana_sees_all(conn).await;
    Ok(found)
}

async fn listed(
    conn: &dyn Connection,
    principals: &str,
    grants: &str,
) -> std::result::Result<Principals, nd_driver::DriverError> {
    let (found, _) = collect(conn, principals, None).await?;
    let (grants, _) = collect(conn, grants, None).await?;
    Ok(Principals {
        principals: found
            .iter()
            .map(|r| Principal {
                name: cell(r, 0).unwrap_or_default(),
                host: None,
                attributes: attributes([
                    (RoleAttribute::Login, yes(r, 1)),
                    (RoleAttribute::Superuser, yes(r, 2)),
                    (RoleAttribute::CreateDb, yes(r, 3)),
                    (RoleAttribute::CreateRole, yes(r, 4)),
                    (RoleAttribute::Locked, yes(r, 6)),
                ]),
                member_of: names(cell(r, 5)),
            })
            .collect(),
        grants: grants.iter().map(|r| grant_of(r, false)).collect(),
        partial: false,
        quotas: Vec::new(),
    })
}

async fn oracle_read(
    conn: &dyn Connection,
) -> std::result::Result<Principals, nd_driver::DriverError> {
    let (found, grants, partial) = match collect(conn, ORA_PRINCIPALS, None).await {
        Ok((found, _)) => (found, collect(conn, ORA_GRANTS, None).await?.0, false),
        Err(_) => (
            collect(conn, ORA_PRINCIPALS_SAFE, None).await?.0,
            collect(conn, ORA_GRANTS_SAFE, None).await?.0,
            true,
        ),
    };
    Ok(Principals {
        principals: found
            .iter()
            .map(|r| Principal {
                name: cell(r, 0).unwrap_or_default(),
                host: None,
                attributes: attributes([
                    (RoleAttribute::Login, yes(r, 1)),
                    (RoleAttribute::Superuser, yes(r, 2)),
                    (RoleAttribute::CreateDb, yes(r, 3)),
                    (RoleAttribute::CreateRole, yes(r, 4)),
                    (RoleAttribute::Locked, yes(r, 6)),
                ]),
                member_of: names(cell(r, 5)),
            })
            .collect(),
        grants: grants.iter().map(|r| grant_of(r, false)).collect(),
        partial,
        quotas: Vec::new(),
    })
}

async fn clickhouse_read(
    conn: &dyn Connection,
) -> std::result::Result<Principals, nd_driver::DriverError> {
    let (users, _) = collect(conn, CH_USERS, None).await?;
    let all = async |sql: &str| {
        collect(conn, sql, None)
            .await
            .map(|(r, _)| r)
            .unwrap_or_default()
    };
    let roles = all(CH_ROLES).await;
    let members = all(CH_ROLE_GRANTS).await;
    let grants = collect(conn, CH_GRANTS, None).await.map(|(r, _)| r);
    let partial = grants.is_err();
    let grants: Vec<Grant> = grants
        .unwrap_or_default()
        .iter()
        .map(clickhouse_grant)
        .collect();
    let quotas = all(CH_QUOTAS).await;
    let limits = all(&ch_quota_limits()).await;
    Ok(Principals {
        principals: clickhouse_principals(&users, &roles, &members, &grants),
        grants,
        partial,
        quotas: clickhouse_quotas(&quotas, &limits),
    })
}

fn clickhouse_grant(r: &Row) -> Grant {
    let database = cell(r, 2);
    let table = cell(r, 3);
    let privilege = match cell(r, 4) {
        Some(col) => format!("{}({col})", cell(r, 1).unwrap_or_default()),
        None => cell(r, 1).unwrap_or_default(),
    };
    let (kind, schema, object) = match (database, table) {
        (None, _) => ("global", None, None),
        (Some(db), None) => ("database", None, Some(db)),
        (Some(db), Some(t)) => ("table", Some(db), Some(t)),
    };
    Grant {
        grantee: cell(r, 0).unwrap_or_default(),
        kind: kind.into(),
        schema,
        object,
        privilege,
        grantable: yes(r, 5),
    }
}

fn clickhouse_principals(
    users: &[Row],
    roles: &[Row],
    members: &[Row],
    grants: &[Grant],
) -> Vec<Principal> {
    let global = |who: &str, any: &[&str]| {
        grants
            .iter()
            .any(|g| g.grantee == who && g.kind == "global" && any.contains(&g.privilege.as_str()))
    };
    let of = |who: &str| -> Vec<String> {
        members
            .iter()
            .filter(|m| cell(m, 0).as_deref() == Some(who))
            .filter_map(|m| cell(m, 1))
            .collect()
    };
    let principal = |name: String, login: bool| {
        let all = global(&name, &["ALL"])
            || ["ACCESS MANAGEMENT", "SYSTEM", "SELECT", "INSERT", "DROP"]
                .iter()
                .all(|p| global(&name, &[p]));
        Principal {
            attributes: attributes([
                (RoleAttribute::Login, login),
                (RoleAttribute::Superuser, all),
                (
                    RoleAttribute::CreateDb,
                    all || global(&name, &["CREATE DATABASE", "CREATE"]),
                ),
                (
                    RoleAttribute::CreateRole,
                    all || global(&name, &["ROLE ADMIN", "CREATE ROLE", "ACCESS MANAGEMENT"]),
                ),
                (RoleAttribute::Locked, false),
            ]),
            member_of: of(&name),
            host: None,
            name,
        }
    };
    users
        .iter()
        .filter_map(|r| cell(r, 0))
        .map(|n| principal(n, true))
        .chain(
            roles
                .iter()
                .filter_map(|r| cell(r, 0))
                .map(|n| principal(n, false)),
        )
        .collect()
}

fn clickhouse_quotas(quotas: &[Row], limits: &[Row]) -> Vec<Quota> {
    let list = |s: Option<String>| -> Vec<String> {
        s.map(|s| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
    };
    quotas
        .iter()
        .map(|q| {
            let name = cell(q, 0).unwrap_or_default();
            let limits = limits
                .iter()
                .filter(|l| cell(l, 0).as_deref() == Some(name.as_str()))
                .map(|l| QuotaLimit {
                    seconds: cell(l, 1).and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    caps: CH_QUOTA_CAPS[..9]
                        .iter()
                        .enumerate()
                        .filter_map(|(i, what)| {
                            let max: f64 = cell(l, i + 2)?.parse().ok()?;
                            Some(QuotaCap {
                                what: (*what).to_owned(),
                                max,
                            })
                        })
                        .collect(),
                })
                .collect();
            Quota {
                all: yes(q, 1),
                applies_to: list(cell(q, 2)),
                except: list(cell(q, 3)),
                limits,
                name,
            }
        })
        .collect()
}

async fn mysql_accounts(
    conn: &dyn Connection,
    grants: &[Grant],
    maria: bool,
) -> (Vec<Principal>, bool) {
    let Ok((rows, _)) = collect(conn, MY_ACCOUNTS, None).await else {
        let mut seen: Vec<String> = grants.iter().map(|g| g.grantee.clone()).collect();
        seen.dedup();
        let list = seen
            .into_iter()
            .map(|g| {
                let (name, host) = g.rsplit_once('@').map_or((g.clone(), None), |(n, h)| {
                    (n.to_owned(), Some(h.to_owned()))
                });
                Principal {
                    name,
                    host,
                    attributes: vec![RoleAttribute::Login],
                    member_of: Vec::new(),
                }
            })
            .collect();
        return (list, true);
    };
    let read = async |sql: &str| {
        collect(conn, sql, None)
            .await
            .map(|(r, _)| r)
            .unwrap_or_default()
    };
    let locked = read(MY_LOCKED).await;
    let (roles, edges) = if maria {
        (read(MARIA_ROLES).await, read(MARIA_EDGES).await)
    } else {
        (read(MY_ROLES).await, read(MY_EDGES).await)
    };
    (mysql_principals(&rows, &locked, &roles, &edges), false)
}

fn mysql_principals(rows: &[Row], locked: &[Row], roles: &[Row], edges: &[Row]) -> Vec<Principal> {
    let same = |a: &Row, b: &Row| cell(a, 0) == cell(b, 0) && cell(a, 1) == cell(b, 1);
    rows.iter()
        .map(|r| {
            let is_locked = locked.iter().any(|l| same(l, r) && yes(l, 2));
            let is_role = roles.iter().any(|x| same(x, r));
            Principal {
                name: cell(r, 0).unwrap_or_default(),
                host: cell(r, 1),
                attributes: attributes([
                    (RoleAttribute::Login, !is_role),
                    (RoleAttribute::Superuser, yes(r, 3)),
                    (RoleAttribute::CreateDb, yes(r, 4)),
                    (RoleAttribute::CreateRole, yes(r, 5)),
                    (RoleAttribute::Locked, is_locked && !is_role),
                ]),
                member_of: edges
                    .iter()
                    .filter(|e| same(e, r))
                    .filter_map(|e| cell(e, 2))
                    .collect(),
            }
        })
        .collect()
}

fn grant_of(r: &Row, mysql: bool) -> Grant {
    let who = cell(r, 0).unwrap_or_default();
    Grant {
        grantee: if mysql { account(&who) } else { who },
        kind: cell(r, 1).unwrap_or_default(),
        schema: cell(r, 2),
        object: cell(r, 3),
        privilege: cell(r, 4).unwrap_or_default(),
        grantable: yes(r, 5),
    }
}
