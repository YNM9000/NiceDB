#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Commit,
    Rollback,
    Begin,
    InTransaction,
    Code,
    Other,
}

fn words(sql: &str, n: usize) -> Vec<String> {
    sql.split(|c: char| c.is_whitespace() || c == '(' || c == ';')
        .filter(|w| !w.is_empty())
        .take(n)
        .map(str::to_ascii_uppercase)
        .collect()
}

fn is_code(w: &[String]) -> bool {
    match w.first().map(String::as_str) {
        Some("DECLARE" | "BEGIN") => true,
        Some("CREATE") => w[1..]
            .iter()
            .find(|x| {
                !matches!(
                    x.as_str(),
                    "OR" | "REPLACE" | "EDITIONABLE" | "NONEDITIONABLE" | "AND" | "COMPILE"
                )
            })
            .is_some_and(|x| {
                matches!(
                    x.as_str(),
                    "PROCEDURE" | "FUNCTION" | "PACKAGE" | "TRIGGER" | "TYPE" | "JAVA" | "LIBRARY"
                )
            }),
        _ => false,
    }
}

pub fn prepare(text: &str) -> (String, Kind) {
    let sql = text.trim();
    let w = words(sql, 6);
    let first = w.first().map_or("", String::as_str);
    let second = w.get(1).map_or("", String::as_str);
    if matches!(first, "EXEC" | "EXECUTE") && !second.is_empty() && second != "IMMEDIATE" {
        let body = sql[first.len()..].trim().trim_end_matches(';').trim_end();
        return (format!("BEGIN {body}; END;"), Kind::Code);
    }
    if is_code(&w) {
        return (sql.to_owned(), Kind::Code);
    }
    let sql = sql.trim_end_matches(';').trim_end().to_owned();
    let kind = match (first, second) {
        ("COMMIT", _) => Kind::Commit,
        ("ROLLBACK", "WORK") if w.get(2).is_some_and(|x| x == "TO") => Kind::InTransaction,
        ("ROLLBACK", "TO") | ("SAVEPOINT", _) => Kind::InTransaction,
        ("ROLLBACK", _) => Kind::Rollback,
        ("SET", "TRANSACTION") => Kind::Begin,
        _ => Kind::Other,
    };
    (sql, kind)
}
