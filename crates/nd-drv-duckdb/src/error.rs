use nd_driver::DriverError;

pub(crate) fn message(e: &duckdb::Error) -> String {
    match e {
        duckdb::Error::DuckDBFailure(_, Some(m)) => m.clone(),
        other => other.to_string(),
    }
}

pub fn map_error(e: &duckdb::Error, sql: &str) -> DriverError {
    let text = message(e);
    let (head, excerpt) = match text.find("\nLINE ") {
        Some(at) => (&text[..at], Some(&text[at + 1..])),
        None => (text.as_str(), None),
    };
    let (kind, rest) = match head.split_once(" Error: ") {
        Some((k, r)) if !k.contains('\n') && !k.is_empty() => (Some(k.trim()), r),
        _ => (None, head),
    };
    if kind.is_some_and(|k| k.eq_ignore_ascii_case("interrupt")) {
        return DriverError::Cancelled;
    }
    let mut lines = rest.trim().lines();
    let message = lines.next().unwrap_or_default().trim().to_owned();
    let detail = lines
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    DriverError::Query {
        message,
        code: kind.map(str::to_owned),
        detail: (!detail.is_empty()).then_some(detail),
        hint: None,
        offset: excerpt.and_then(|x| offset(x, sql)),
    }
}

pub(crate) fn shifted(e: DriverError, prefix: usize) -> DriverError {
    match e {
        DriverError::Query {
            message,
            code,
            detail,
            hint,
            offset,
        } => DriverError::Query {
            message,
            code,
            detail,
            hint,
            offset: offset.and_then(|o| o.checked_sub(u32::try_from(prefix).ok()?)),
        },
        other => other,
    }
}

fn offset(excerpt: &str, sql: &str) -> Option<u32> {
    let mut lines = excerpt.lines();
    let first = lines.next()?.strip_prefix("LINE ")?;
    let caret = lines.next()?.find('^')?;
    let (n, shown) = first.split_once(": ")?;
    let n: usize = n.trim().parse().ok()?;
    let prefix = "LINE ".len() + n.to_string().len() + 2;
    let col = caret.checked_sub(prefix)?;
    let line_start: usize = sql
        .split_inclusive('\n')
        .take(n.saturating_sub(1))
        .map(str::len)
        .sum();
    let line = sql.get(line_start..)?;
    let line = &line[..line.find('\n').unwrap_or(line.len())];
    let (shown, col) = match shown.strip_prefix("...") {
        Some(s) => (s, col.checked_sub(3)?),
        None => (shown, col),
    };
    let shown = shown.trim_end_matches("...");
    let at = line.find(&shown[..shown.floor_char_boundary(40)])? + col;
    (at <= line.len()).then(|| u32::try_from(line_start + at).ok())?
}
