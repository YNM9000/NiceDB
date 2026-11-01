use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Check {
    Values {
        values: Vec<String>,
    },
    Range {
        min: Option<f64>,
        max: Option<f64>,
        min_open: bool,
        max_open: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Ident(String),
    Str(String),
    Num(f64),
    Op(String),
    Word(String),
    Comma,
}

const WORDS: &[&str] = &[
    "and", "or", "in", "between", "any", "array", "is", "null", "not", "check",
];
const TYPE_WORDS: &[&str] = &["varying", "precision", "without", "with", "time", "zone"];

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn quoted(chars: &[char], i: &mut usize, close: char) -> String {
    let mut s = String::new();
    *i += 1;
    while *i < chars.len() {
        let c = chars[*i];
        if c == '\\' && close == '\'' && *i + 1 < chars.len() {
            s.push(chars[*i + 1]);
            *i += 2;
            continue;
        }
        if c == close {
            if chars.get(*i + 1) == Some(&close) {
                s.push(close);
                *i += 2;
                continue;
            }
            *i += 1;
            return s;
        }
        s.push(c);
        *i += 1;
    }
    s
}

#[allow(clippy::too_many_lines)]
fn tokens(expr: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = expr.replace("\\'", "'").chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() || c == '(' || c == ')' || c == ']' => i += 1,
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '\'' => {
                let s = quoted(&chars, &mut i, '\'');
                out.push(Tok::Str(s));
            }
            '"' | '`' => {
                let s = quoted(&chars, &mut i, c);
                out.push(Tok::Ident(s.to_lowercase()));
            }
            '[' => {
                let end = chars[i..].iter().position(|&c| c == ']').map(|p| i + p);
                match end {
                    Some(e)
                        if e > i + 1 && chars[i + 1..e].iter().all(|&c| c != '\'' && c != ',') =>
                    {
                        let s: String = chars[i + 1..e].iter().collect();
                        out.push(Tok::Ident(s.to_lowercase()));
                        i = e + 1;
                    }
                    _ => i += 1,
                }
            }
            ':' => {
                while i < chars.len() && chars[i] == ':' {
                    i += 1;
                }
                loop {
                    while i < chars.len() && chars[i].is_whitespace() {
                        i += 1;
                    }
                    let start = i;
                    while i < chars.len() && ident_char(chars[i]) {
                        i += 1;
                    }
                    let word: String = chars[start..i].iter().collect::<String>().to_lowercase();
                    if word.is_empty() {
                        break;
                    }
                    let next_start = {
                        let mut j = i;
                        while j < chars.len() && chars[j].is_whitespace() {
                            j += 1;
                        }
                        let s = j;
                        while j < chars.len() && ident_char(chars[j]) {
                            j += 1;
                        }
                        chars[s..j].iter().collect::<String>().to_lowercase()
                    };
                    if !TYPE_WORDS.contains(&next_start.as_str()) {
                        break;
                    }
                }
                if chars.get(i) == Some(&'[') && chars.get(i + 1) == Some(&']') {
                    i += 2;
                }
            }
            '<' | '>' | '=' | '!' => {
                let mut op = c.to_string();
                if let Some(&n) = chars.get(i + 1)
                    && (n == '=' || (c == '<' && n == '>'))
                {
                    op.push(n);
                    i += 1;
                }
                i += 1;
                out.push(Tok::Op(if op == "!=" { "<>".into() } else { op }));
            }
            c if c.is_ascii_digit()
                || ((c == '-' || c == '.')
                    && chars.get(i + 1).is_some_and(char::is_ascii_digit)) =>
            {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                out.push(Tok::Num(s.parse().ok()?));
            }
            c if ident_char(c) => {
                let start = i;
                while i < chars.len() && ident_char(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let lower = word.to_lowercase();
                if chars.get(i) == Some(&'\'')
                    && (lower.starts_with('_') || lower == "n" || lower == "e")
                {
                    continue;
                }
                if WORDS.contains(&lower.as_str()) {
                    out.push(Tok::Word(lower));
                } else {
                    out.push(Tok::Ident(lower));
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

fn literal(t: &Tok) -> Option<String> {
    match t {
        Tok::Str(s) => Some(s.clone()),
        Tok::Num(n) => Some(number(*n)),
        _ => None,
    }
}

fn number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

#[derive(Default)]
struct Found {
    values: Vec<String>,
    min: Option<(f64, bool)>,
    max: Option<(f64, bool)>,
    or: bool,
    and: bool,
}

impl Found {
    fn lower(&mut self, v: f64, open: bool) {
        if self.min.is_none_or(|(m, _)| v > m) {
            self.min = Some((v, open));
        }
    }

    fn upper(&mut self, v: f64, open: bool) {
        if self.max.is_none_or(|(m, _)| v < m) {
            self.max = Some((v, open));
        }
    }

    fn compare(&mut self, op: &str, v: &Tok, flipped: bool) -> Option<()> {
        let op = match (op, flipped) {
            ("<", true) => ">",
            (">", true) => "<",
            ("<=", true) => ">=",
            (">=", true) => "<=",
            (o, _) => o,
        };
        if op == "=" {
            self.values.push(literal(v)?);
            return Some(());
        }
        let Tok::Num(n) = v else { return None };
        match op {
            ">" => self.lower(*n, true),
            ">=" => self.lower(*n, false),
            "<" => self.upper(*n, true),
            "<=" => self.upper(*n, false),
            _ => return None,
        }
        Some(())
    }
}

fn list(t: &[Tok], mut i: usize, out: &mut Vec<String>) -> Option<usize> {
    loop {
        out.push(literal(t.get(i)?)?);
        i += 1;
        if t.get(i) == Some(&Tok::Comma) {
            i += 1;
        } else {
            return Some(i);
        }
    }
}

pub fn parse_check(expr: &str, column: &str) -> Option<Check> {
    let toks = tokens(expr)?;
    let col = column.to_lowercase();
    let me = |tok: Option<&Tok>| matches!(tok, Some(Tok::Ident(n)) if *n == col);
    let word = |tok: Option<&Tok>, word: &str| matches!(tok, Some(Tok::Word(x)) if x == word);
    let mut found = Found::default();
    let mut i = usize::from(word(toks.first(), "check"));
    if i >= toks.len() {
        return None;
    }
    while i < toks.len() {
        if me(toks.get(i)) && word(toks.get(i + 1), "is") && word(toks.get(i + 2), "null") {
            i += 3;
        } else if me(toks.get(i)) && word(toks.get(i + 1), "in") {
            i = list(&toks, i + 2, &mut found.values)?;
        } else if me(toks.get(i))
            && toks.get(i + 1) == Some(&Tok::Op("=".into()))
            && word(toks.get(i + 2), "any")
        {
            let start = i + 3 + usize::from(word(toks.get(i + 3), "array"));
            i = list(&toks, start, &mut found.values)?;
        } else if me(toks.get(i))
            && word(toks.get(i + 1), "between")
            && word(toks.get(i + 3), "and")
        {
            let (Some(Tok::Num(a)), Some(Tok::Num(b))) = (toks.get(i + 2), toks.get(i + 4)) else {
                return None;
            };
            found.lower(*a, false);
            found.upper(*b, false);
            found.and = true;
            i += 5;
        } else if let (true, Some(Tok::Op(op)), Some(v)) =
            (me(toks.get(i)), toks.get(i + 1), toks.get(i + 2))
        {
            found.compare(op, v, false)?;
            i += 3;
        } else if let (Some(v), Some(Tok::Op(op)), true) =
            (toks.get(i), toks.get(i + 1), me(toks.get(i + 2)))
        {
            found.compare(op, v, true)?;
            i += 3;
        } else {
            return None;
        }
        match toks.get(i) {
            None => break,
            Some(Tok::Word(x)) if x == "or" => found.or = true,
            Some(Tok::Word(x)) if x == "and" => found.and = true,
            _ => return None,
        }
        i += 1;
    }
    let ranged = found.min.is_some() || found.max.is_some();
    match (found.values.is_empty(), ranged) {
        (false, false) if !found.and => Some(Check::Values {
            values: found.values,
        }),
        (true, true) if !found.or => Some(Check::Range {
            min: found.min.map(|m| m.0),
            max: found.max.map(|m| m.0),
            min_open: found.min.is_some_and(|m| m.1),
            max_open: found.max.is_some_and(|m| m.1),
        }),
        _ => None,
    }
}
