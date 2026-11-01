use crate::sqltext::{Tok, tokens};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Use {
    Plain { with: Operand },
    Function { name: String, call: String },
    Cast { to: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Operand {
    Number,
    Text,
    Column(Option<String>),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnUse {
    pub qualifier: Option<String>,
    pub column: String,
    pub how: Use,
}

const KEYWORDS: &[&str] = &[
    "AND",
    "OR",
    "NOT",
    "NULL",
    "TRUE",
    "FALSE",
    "IS",
    "LIKE",
    "ILIKE",
    "IN",
    "BETWEEN",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "ANY",
    "ALL",
    "SOME",
    "ARRAY",
    "DATE",
    "TIMESTAMP",
    "INTERVAL",
    "FROM",
    "AS",
    "DESC",
    "ASC",
    "SUBPLAN",
    "INITPLAN",
    "HASHED",
    "DISTINCT",
    "EXISTS",
    "COLLATE",
    "ESCAPE",
    "TIME",
    "ZONE",
    "AT",
    "SIMILAR",
    "TO",
    "DOW",
    "YEAR",
    "MONTH",
    "DAY",
    "EPOCH",
    "HOUR",
    "MINUTE",
    "SECOND",
    "WEEK",
];
const TYPE_TAIL: &[&str] = &["VARYING", "PRECISION", "WITH", "WITHOUT", "TIME", "ZONE"];
const COMPARE: &[&str] = &["=", "<>", "!=", "<", ">", "<=", ">=", "~~"];

fn keyword(w: &str) -> bool {
    KEYWORDS.iter().any(|k| w.eq_ignore_ascii_case(k))
}

fn call_text(t: &[Tok], at: usize) -> String {
    let mut out = String::new();
    let mut depth = 0i32;
    for tok in &t[at..] {
        match tok {
            Tok::Word(w) => {
                if out.ends_with(|c: char| c.is_alphanumeric() || c == '_' || c == ')') {
                    out.push(' ');
                }
                out.push_str(w);
            }
            Tok::Quoted(w) => out.push_str(w),
            Tok::Num(n) => {
                if out.ends_with(|c: char| c.is_alphanumeric()) {
                    out.push(' ');
                }
                out.push_str(n);
            }
            Tok::Str(q) => out.push_str(q),
            Tok::Punct(p) => {
                if p == "(" {
                    depth += 1;
                } else if p == ")" {
                    depth -= 1;
                }
                if p == "," {
                    out.push_str(", ");
                } else {
                    out.push_str(p);
                }
                if depth == 0 && p == ")" {
                    break;
                }
            }
        }
    }
    out
}

fn operand(t: &[Tok], at: usize) -> Operand {
    match t.get(at) {
        Some(Tok::Num(_)) => Operand::Number,
        Some(Tok::Str(_)) => Operand::Text,
        Some(Tok::Word(w))
            if (w.eq_ignore_ascii_case("DATE") || w.eq_ignore_ascii_case("TIMESTAMP"))
                && matches!(t.get(at + 1), Some(Tok::Str(_))) =>
        {
            Operand::Text
        }
        Some(Tok::Punct(p)) if p == "-" && matches!(t.get(at + 1), Some(Tok::Num(_))) => {
            Operand::Number
        }
        Some(Tok::Punct(p)) if p == "(" => operand(t, at + 1),
        Some(Tok::Word(w) | Tok::Quoted(w)) if !keyword(w) => {
            if t.get(at + 1).is_some_and(|x| x.is(".")) {
                Operand::Column(Some(w.clone()))
            } else if t.get(at + 1).is_some_and(|x| x.is("(")) {
                Operand::Other
            } else {
                Operand::Column(None)
            }
        }
        _ => Operand::Other,
    }
}

fn classify(t: &[Tok], i: usize, next: usize, enclosing: Option<usize>) -> Use {
    if let Some(at) = enclosing {
        let name = t[at].word().unwrap_or_default().to_ascii_lowercase();
        if name == "cast" || name == "convert" {
            let to = t[next..]
                .iter()
                .skip_while(|x| !x.is_kw("AS"))
                .nth(1)
                .and_then(Tok::word)
                .unwrap_or_default();
            Use::Cast {
                to: to.to_ascii_lowercase(),
            }
        } else {
            Use::Function {
                name,
                call: call_text(t, at),
            }
        }
    } else if t.get(next).is_some_and(|x| x.is("::")) {
        Use::Cast {
            to: t
                .get(next + 1)
                .and_then(Tok::word)
                .unwrap_or_default()
                .to_ascii_lowercase(),
        }
    } else if t.get(next).is_some_and(|x| x.is(")"))
        && t.get(next + 1).is_some_and(|x| x.is("::"))
        && i > 0
        && t[i - 1].is("(")
    {
        Use::Cast {
            to: t
                .get(next + 2)
                .and_then(Tok::word)
                .unwrap_or_default()
                .to_ascii_lowercase(),
        }
    } else {
        let with = match t.get(next) {
            Some(Tok::Punct(p)) if COMPARE.contains(&p.as_str()) => operand(t, next + 1),
            Some(x) if x.is_kw("LIKE") || x.is_kw("IN") => operand(t, next + 1),
            _ if i > 0 && matches!(&t[i - 1], Tok::Punct(p) if COMPARE.contains(&p.as_str())) => {
                operand(t, i.saturating_sub(2))
            }
            _ => Operand::Other,
        };
        Use::Plain { with }
    }
}

pub(crate) fn columns(cond: &str) -> Vec<ColumnUse> {
    let t = tokens(cond);
    let mut out = Vec::new();
    let mut calls: Vec<Option<usize>> = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let tok = &t[i];
        if tok.is("(") {
            calls.push(None);
            i += 1;
            continue;
        }
        if tok.is(")") {
            calls.pop();
            i += 1;
            continue;
        }
        if tok.is("::") {
            i += 2;
            while t
                .get(i)
                .and_then(Tok::word)
                .is_some_and(|w| TYPE_TAIL.iter().any(|k| w.eq_ignore_ascii_case(k)))
            {
                i += 1;
            }
            continue;
        }
        let Some(word) = tok.word() else {
            i += 1;
            continue;
        };
        if matches!(tok, Tok::Word(_)) && keyword(word) {
            i += 1;
            continue;
        }
        if t.get(i + 1).is_some_and(|x| x.is("(")) {
            calls.push(Some(i));
            i += 2;
            continue;
        }
        let mut parts = vec![word.to_owned()];
        let mut j = i;
        while t.get(j + 1).is_some_and(|x| x.is(".")) {
            match t.get(j + 2).and_then(Tok::word) {
                Some(w) => parts.push(w.to_owned()),
                None => break,
            }
            j += 2;
        }
        let column = parts.pop().unwrap_or_default();
        let qualifier = parts.pop();
        let next = j + 1;
        let enclosing = calls.iter().rev().flatten().next().copied();
        let how = classify(&t, i, next, enclosing);
        out.push(ColumnUse {
            qualifier,
            column,
            how,
        });
        i = next;
    }
    out
}
