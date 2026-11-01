use std::ops::Range;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    Postgres,
    Sqlite,
    Mysql,
    Mssql,
    Duckdb,
    Clickhouse,
    Firebird,
    Hana,
    Db2,
    Ansi,
    #[serde(rename = "ansi_backtick")]
    AnsiBacktick,
    Oracle,
    Cql,
    Partiql,
    Opensearch,
    Elasticsearch,
}

impl Dialect {
    pub fn is_generic(self) -> bool {
        matches!(self, Self::Ansi | Self::AnsiBacktick)
    }

    pub(crate) fn nested_comments(self) -> bool {
        matches!(
            self,
            Self::Postgres | Self::Mssql | Self::Duckdb | Self::Clickhouse
        )
    }

    pub(crate) fn dollar_quotes(self) -> bool {
        matches!(
            self,
            Self::Postgres | Self::Duckdb | Self::Clickhouse | Self::Cql
        )
    }

    pub(crate) fn hash_comments(self) -> bool {
        matches!(self, Self::Mysql | Self::Clickhouse)
    }

    pub(crate) fn backslash_escapes(self, quote: u8) -> bool {
        match self {
            Self::Mysql => quote != b'`',
            Self::Clickhouse => true,
            _ => false,
        }
    }

    pub(crate) fn double_quotes_name(self) -> bool {
        !matches!(self, Self::Mysql | Self::Opensearch | Self::AnsiBacktick)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub range: Range<usize>,
}

impl Statement {
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.range.clone()]
    }
}

#[allow(clippy::too_many_lines)]
pub fn split(sql: &str, dialect: Dialect) -> Vec<Statement> {
    let b = sql.as_bytes();
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut has_code = false;
    let mut block_depth = 0usize;
    let mut words = Words::default();
    let mut delimiter: Option<&[u8]> = None;
    let mut batch = Batch::default();

    while i < b.len() {
        let c = b[i];
        if let Some(d) = delimiter.filter(|d| b[i..].starts_with(d)) {
            push(&mut out, sql, start, i, has_code);
            i += d.len();
            start = i;
            has_code = false;
            words = Words::default();
            continue;
        }
        if c == b'/'
            && dialect == Dialect::Oracle
            && let Some((line_start, next)) = slash_line(b, i)
        {
            push(&mut out, sql, start, line_start, has_code);
            (i, start, has_code, block_depth) = (next, next, false, 0);
            words = Words::default();
            continue;
        }
        match c {
            b'-' | b'#' if line_comment(b, i, dialect) => {
                i = memchr(b'\n', b, i).unwrap_or(b.len());
                continue;
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                if dialect == Dialect::Mysql && matches!(b.get(i + 2), Some(b'!' | b'+')) {
                    has_code = true;
                }
                i = skip_block_comment(b, i + 2, dialect.nested_comments());
                continue;
            }
            b'\'' | b'"' | b'`' => {
                has_code = true;
                i = skip_quoted(b, i + 1, c, dialect.backslash_escapes(c));
                continue;
            }
            b'[' if matches!(dialect, Dialect::Sqlite | Dialect::Mssql) => {
                has_code = true;
                i = skip_bracket(b, i, dialect);
                continue;
            }
            b'$' if dialect.dollar_quotes() => {
                if let Some(tag_end) = dollar_tag(b, i) {
                    has_code = true;
                    let tag = &b[i..=tag_end];
                    i = find(b, tag, tag_end + 1).map_or(b.len(), |e| e + tag.len());
                    continue;
                }
            }
            b';' if dialect == Dialect::Oracle && words.ora.active => {
                if words.ora.semicolon() {
                    push(&mut out, sql, start, i + 1, has_code);
                    start = i + 1;
                    has_code = false;
                    words = Words::default();
                }
            }
            b';' if block_depth == 0 && delimiter.is_none() && !batch.whole => {
                push(&mut out, sql, start, i, has_code);
                start = i + 1;
                has_code = false;
                words = Words::default();
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let end = b[i..]
                    .iter()
                    .position(|x| !(x.is_ascii_alphanumeric() || *x == b'_'))
                    .map_or(b.len(), |p| i + p);
                if dialect == Dialect::Oracle
                    && let Some(next) = q_quote(b, i, end)
                {
                    has_code = true;
                    i = next;
                    continue;
                }
                if !has_code
                    && let Some((d, next)) = match dialect {
                        Dialect::Mysql => delimiter_command(b, i, end),
                        Dialect::Firebird => set_term(b, i, end),
                        _ => None,
                    }
                {
                    delimiter = (d != b";").then_some(d);
                    i = next;
                    start = next;
                    continue;
                }
                if dialect == Dialect::Mssql
                    && let Some((line_start, next, times)) = go_line(b, i, end)
                {
                    push(&mut out, sql, start, line_start, has_code);
                    batch.close(&mut out, times);
                    (i, start, has_code, block_depth) = (next, next, false, 0);
                    words = Words::default();
                    continue;
                }
                has_code = true;
                block_depth = words.track_for(dialect, &sql[i..end], block_depth, &mut batch);
                i = end;
                continue;
            }
            _ if !c.is_ascii_whitespace() => has_code = true,
            _ => {}
        }
        i += 1;
    }
    push(&mut out, sql, start, b.len(), has_code);
    batch.close(&mut out, 1);
    out
}

pub(crate) fn line_comment(b: &[u8], i: usize, dialect: Dialect) -> bool {
    match b[i] {
        b'#' => dialect.hash_comments(),
        _ => {
            b.get(i + 1) == Some(&b'-')
                && (dialect != Dialect::Mysql
                    || b.get(i + 2)
                        .is_none_or(|x| x.is_ascii_whitespace() || x.is_ascii_control()))
        }
    }
}

pub(crate) fn skip_bracket(b: &[u8], i: usize, dialect: Dialect) -> usize {
    if dialect == Dialect::Mssql {
        skip_quoted(b, i + 1, b']', false)
    } else {
        memchr(b']', b, i + 1).map_or(b.len(), |e| e + 1)
    }
}

#[derive(Default)]
struct Batch {
    opened: usize,
    whole: bool,
    declares: bool,
}

impl Batch {
    fn close(&mut self, out: &mut Vec<Statement>, times: usize) {
        if self.declares && out.len() > self.opened + 1 {
            let end = out.last().map_or(0, |s| s.range.end);
            let first = out[self.opened].range.start;
            out.truncate(self.opened);
            out.push(Statement { range: first..end });
        }
        let batch = out[self.opened..].to_vec();
        for _ in 1..times {
            out.extend_from_slice(&batch);
        }
        *self = Self {
            opened: out.len(),
            ..Self::default()
        };
    }
}

const MAX_GO_COUNT: usize = 1000;

fn go_line(b: &[u8], i: usize, end: usize) -> Option<(usize, usize, usize)> {
    if !b[i..end].eq_ignore_ascii_case(b"go") {
        return None;
    }
    let line_start = b[..i]
        .iter()
        .rposition(|&x| x == b'\n')
        .map_or(0, |p| p + 1);
    if !b[line_start..i].iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    let line_end = memchr(b'\n', b, end).unwrap_or(b.len());
    let rest = b[end..line_end].trim_ascii();
    let digits = rest.iter().take_while(|x| x.is_ascii_digit()).count();
    let tail = rest[digits..].trim_ascii();
    let times = std::str::from_utf8(&rest[..digits])
        .ok()
        .and_then(|d| d.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, MAX_GO_COUNT);
    (tail.is_empty() || tail.starts_with(b"--"))
        .then(|| (line_start, (line_end + 1).min(b.len()), times))
}

fn delimiter_command(b: &[u8], i: usize, end: usize) -> Option<(&[u8], usize)> {
    if !b[i..end].eq_ignore_ascii_case(b"delimiter") {
        return None;
    }
    let line_start = b[..i]
        .iter()
        .rposition(|&x| x == b'\n')
        .map_or(0, |p| p + 1);
    if !b[line_start..i].iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    let line_end = memchr(b'\n', b, end).unwrap_or(b.len());
    let rest = &b[end..line_end];
    if !rest.first().is_some_and(|x| *x == b' ' || *x == b'\t') {
        return None;
    }
    let token = rest.trim_ascii().split(u8::is_ascii_whitespace).next()?;
    (!token.is_empty()).then(|| (token, (line_end + 1).min(b.len())))
}

fn slash_line(b: &[u8], i: usize) -> Option<(usize, usize)> {
    let line_start = b[..i]
        .iter()
        .rposition(|&x| x == b'\n')
        .map_or(0, |p| p + 1);
    let line_end = memchr(b'\n', b, i).unwrap_or(b.len());
    (b[line_start..i].iter().all(u8::is_ascii_whitespace)
        && b[i + 1..line_end].iter().all(u8::is_ascii_whitespace))
    .then(|| (line_start, (line_end + 1).min(b.len())))
}

pub(crate) fn q_quote(b: &[u8], i: usize, end: usize) -> Option<usize> {
    let prefix = &b[i..end];
    if !(prefix.eq_ignore_ascii_case(b"q") || prefix.eq_ignore_ascii_case(b"nq"))
        || b.get(end) != Some(&b'\'')
    {
        return None;
    }
    let open = *b.get(end + 1)?;
    let close = match open {
        b'[' => b']',
        b'{' => b'}',
        b'(' => b')',
        b'<' => b'>',
        c if c.is_ascii_whitespace() => return None,
        c => c,
    };
    let mut j = end + 2;
    while j + 1 < b.len() {
        if b[j] == close && b[j + 1] == b'\'' {
            return Some(j + 2);
        }
        j += 1;
    }
    Some(b.len())
}

fn set_term(b: &[u8], i: usize, end: usize) -> Option<(&[u8], usize)> {
    if !b[i..end].eq_ignore_ascii_case(b"set") {
        return None;
    }
    let line_end = memchr(b'\n', b, end).unwrap_or(b.len());
    let mut parts = b[end..line_end]
        .split(u8::is_ascii_whitespace)
        .filter(|p| !p.is_empty());
    if !parts.next()?.eq_ignore_ascii_case(b"term") {
        return None;
    }
    let token = parts.next()?;
    let token = token
        .strip_suffix(b";")
        .filter(|t| !t.is_empty())
        .unwrap_or(token);
    Some((token, (line_end + 1).min(b.len())))
}

pub fn statement_at(statements: &[Statement], cursor: usize) -> Option<&Statement> {
    statements
        .iter()
        .rev()
        .find(|s| s.range.start <= cursor)
        .or_else(|| statements.first())
}

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct Words {
    ora: Plsql,
    in_trigger: bool,
    create: Option<bool>,
    after_end: bool,
    seen: usize,
    after_begin: bool,
    first_was_execute: bool,
}

impl Words {
    fn track_for(
        &mut self,
        dialect: Dialect,
        word: &str,
        depth: usize,
        batch: &mut Batch,
    ) -> usize {
        match dialect {
            Dialect::Sqlite => self.track(word, depth),
            Dialect::Mysql => self.track_mysql(word, depth),
            Dialect::Mssql => self.track_mssql(word, depth, batch),
            Dialect::Firebird => self.track_psql(word, depth, false),
            Dialect::Hana | Dialect::Db2 => self.track_psql(word, depth, true),
            Dialect::Oracle => self.ora.track(word),
            Dialect::Postgres
            | Dialect::Duckdb
            | Dialect::Clickhouse
            | Dialect::Ansi
            | Dialect::AnsiBacktick
            | Dialect::Cql
            | Dialect::Partiql
            | Dialect::Opensearch
            | Dialect::Elasticsearch => depth,
        }
    }

    fn track(&mut self, word: &str, depth: usize) -> usize {
        if word.eq_ignore_ascii_case("trigger") {
            self.in_trigger = true;
        }
        if self.in_trigger && word.eq_ignore_ascii_case("begin") {
            return depth + 1;
        }
        if depth > 0 && word.eq_ignore_ascii_case("end") {
            if depth == 1 {
                self.in_trigger = false;
            }
            return depth - 1;
        }
        if depth > 0 && word.eq_ignore_ascii_case("case") {
            return depth + 1;
        }
        depth
    }

    fn track_mysql(&mut self, word: &str, depth: usize) -> usize {
        let is = |w: &str| word.eq_ignore_ascii_case(w);
        let create = *self.create.get_or_insert_with(|| is("create"));
        let after_end = std::mem::take(&mut self.after_end);
        if after_end && ["if", "loop", "while", "repeat"].into_iter().any(is) {
            return depth + 1;
        }
        if create
            && ["procedure", "function", "trigger", "event"]
                .into_iter()
                .any(is)
        {
            self.in_trigger = true;
        }
        if self.in_trigger && is("begin") {
            return depth + 1;
        }
        if depth > 0 && is("end") {
            self.after_end = true;
            return depth - 1;
        }
        if depth > 0 && !after_end && is("case") {
            return depth + 1;
        }
        depth
    }
}

impl Words {
    fn track_psql(&mut self, word: &str, depth: usize, hana: bool) -> usize {
        let is = |w: &str| word.eq_ignore_ascii_case(w);
        let n = self.seen;
        self.seen += 1;
        if n == 0 {
            self.create = Some(
                ["create", "recreate", "alter", "execute"]
                    .into_iter()
                    .any(is),
            );
            if hana && is("do") {
                self.in_trigger = true;
            }
        } else if self.create == Some(true)
            && !self.in_trigger
            && n <= 4
            && ["procedure", "function", "trigger", "package", "block"]
                .into_iter()
                .any(is)
        {
            self.in_trigger = !(n == 1 && is("procedure") && self.first_was_execute);
            return depth;
        }
        if n == 0 {
            self.first_was_execute = is("execute");
        }
        if !self.in_trigger {
            return depth;
        }
        let after_end = std::mem::take(&mut self.after_end);
        if hana && after_end && ["if", "for", "while", "loop", "case"].into_iter().any(is) {
            return depth + 1;
        }
        if !hana && depth == 0 && !self.after_begin && is("as") {
            self.after_begin = true;
            return 1;
        }
        if is("begin") {
            if std::mem::take(&mut self.after_begin) {
                return depth;
            }
            return depth + 1;
        }
        if is("case") {
            return depth + 1;
        }
        if depth > 0 && is("end") {
            self.after_end = true;
            return depth - 1;
        }
        depth
    }

    fn track_mssql(&mut self, word: &str, depth: usize, batch: &mut Batch) -> usize {
        let is = |w: &str| word.eq_ignore_ascii_case(w);
        let n = self.seen;
        self.seen += 1;
        if is("declare") {
            batch.declares = true;
        }
        if n == 0 {
            self.create = Some(is("create") || is("alter"));
        } else if self.create == Some(true)
            && n <= 3
            && ["procedure", "proc", "function", "trigger", "view"]
                .into_iter()
                .any(is)
        {
            batch.whole = true;
        }
        let mut depth = depth;
        if std::mem::take(&mut self.after_begin)
            && ![
                "tran",
                "transaction",
                "distributed",
                "dialog",
                "conversation",
            ]
            .into_iter()
            .any(is)
        {
            depth += 1;
        }
        if is("begin") {
            self.after_begin = true;
        } else if is("case") {
            depth += 1;
        } else if is("end") {
            depth = depth.saturating_sub(1);
        }
        depth
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Declare,
    Block,
    Case,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Unit {
    #[default]
    None,
    Declared,
    Trigger,
    Opaque,
}

#[derive(Default)]
#[allow(clippy::struct_excessive_bools)]
struct Plsql {
    active: bool,
    seen: usize,
    create: bool,
    unit: Unit,
    stack: Vec<Level>,
    opened: bool,
    heading: bool,
    pending_end: bool,
    type_word: bool,
    done: bool,
}

impl Plsql {
    fn resolve_end(&mut self, next: Option<&str>) -> bool {
        if !std::mem::take(&mut self.pending_end) {
            return false;
        }
        let closes_nothing =
            next.is_some_and(|w| ["if", "loop"].iter().any(|k| w.eq_ignore_ascii_case(k)));
        if !closes_nothing {
            self.stack.pop();
            if self.stack.is_empty() && self.opened {
                self.done = true;
            }
        }
        next.is_some_and(|w| w.eq_ignore_ascii_case("case"))
    }

    fn semicolon(&mut self) -> bool {
        self.resolve_end(None);
        self.heading = false;
        self.done && self.unit != Unit::Opaque
    }

    fn track(&mut self, word: &str) -> usize {
        let is = |w: &str| word.eq_ignore_ascii_case(w);
        let n = self.seen;
        self.seen += 1;
        if n == 0 {
            if is("declare") || is("begin") {
                self.active = true;
                self.opened = true;
                self.stack.push(if is("begin") {
                    Level::Block
                } else {
                    Level::Declare
                });
            }
            self.create = is("create");
            return self.depth();
        }
        if self.create && !self.active {
            if [
                "or",
                "replace",
                "editionable",
                "noneditionable",
                "and",
                "compile",
                "resolve",
                "noforce",
                "force",
            ]
            .iter()
            .any(|k| is(k))
            {
                return 0;
            }
            self.create = false;
            self.unit = if is("procedure") || is("function") || is("package") {
                Unit::Declared
            } else if is("trigger") {
                Unit::Trigger
            } else if is("type") || is("java") {
                self.type_word = is("type");
                Unit::Opaque
            } else {
                return 0;
            };
            self.active = true;
            return self.depth();
        }
        if !self.active || self.done {
            return self.depth();
        }
        if std::mem::take(&mut self.type_word) && is("body") {
            self.unit = Unit::Declared;
        }
        if self.unit == Unit::Trigger && is("compound") {
            self.unit = Unit::Opaque;
        }
        if self.resolve_end(Some(word)) {
            return self.depth();
        }
        if is("end") {
            self.pending_end = true;
        } else if is("case") {
            self.stack.push(Level::Case);
        } else if is("begin") {
            self.heading = false;
            self.opened = true;
            match self.stack.last_mut() {
                Some(top @ Level::Declare) => *top = Level::Block,
                _ => self.stack.push(Level::Block),
            }
        } else if (is("declare") && self.unit == Unit::Trigger && self.stack.is_empty())
            || ((is("is") || is("as")) && self.unit == Unit::Declared && !self.opened)
        {
            self.opened = true;
            self.stack.push(Level::Declare);
        } else if (is("is") || is("as")) && self.heading {
            self.heading = false;
            self.stack.push(Level::Declare);
        } else if (is("procedure") || is("function")) && self.stack.last() == Some(&Level::Declare)
        {
            self.heading = true;
        }
        self.depth()
    }

    fn depth(&self) -> usize {
        usize::from(self.active)
    }
}

fn push(out: &mut Vec<Statement>, sql: &str, start: usize, end: usize, has_code: bool) {
    if !has_code {
        return;
    }
    let raw = &sql[start..end];
    let lead = raw.len() - raw.trim_ascii_start().len();
    let trail = raw.len() - raw.trim_ascii_end().len();
    out.push(Statement {
        range: start + lead..end - trail,
    });
}

pub(crate) fn memchr(needle: u8, b: &[u8], from: usize) -> Option<usize> {
    b[from..]
        .iter()
        .position(|&x| x == needle)
        .map(|p| from + p)
}

pub(crate) fn find(b: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    b.get(from..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| from + p)
}

pub(crate) fn skip_quoted(b: &[u8], mut i: usize, q: u8, backslash: bool) -> usize {
    while i < b.len() {
        if backslash && b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == q {
            if b.get(i + 1) == Some(&q) {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    b.len()
}

pub(crate) fn skip_block_comment(b: &[u8], mut i: usize, nested: bool) -> usize {
    let mut depth = 1;
    while i < b.len() {
        if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
            continue;
        }
        if nested && b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
            depth += 1;
            i += 2;
            continue;
        }
        i += 1;
    }
    b.len()
}

pub(crate) fn dollar_tag(b: &[u8], start: usize) -> Option<usize> {
    if start > 0 && (b[start - 1].is_ascii_alphanumeric() || b[start - 1] == b'_') {
        return None;
    }
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'$' => return Some(i),
            c if c.is_ascii_alphanumeric() || c == b'_' => {
                if i == start + 1 && c.is_ascii_digit() {
                    return None;
                }
                i += 1;
            }
            _ => return None,
        }
    }
    None
}
