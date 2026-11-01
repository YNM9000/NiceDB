use std::fmt::Write as _;

use crate::Rng;
use crate::data::{ALNUM, DIGITS};

const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";

#[derive(Debug, Clone, PartialEq)]
enum Filter {
    Lower,
    Upper,
    Initial,
    Slug,
    Year,
    Month,
    Day,
    Trim,
}

impl Filter {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "lower" | "minuscolo" => Self::Lower,
            "upper" | "maiuscolo" => Self::Upper,
            "initial" | "iniziale" => Self::Initial,
            "slug" => Self::Slug,
            "year" | "anno" => Self::Year,
            "month" | "mese" => Self::Month,
            "day" | "giorno" => Self::Day,
            "trim" => Self::Trim,
            _ => return None,
        })
    }

    fn apply(&self, v: &str) -> String {
        match self {
            Self::Lower => v.to_lowercase(),
            Self::Upper => v.to_uppercase(),
            Self::Initial => v.chars().next().map(String::from).unwrap_or_default(),
            Self::Slug => crate::plain(v),
            Self::Year => v.get(..4).unwrap_or(v).to_owned(),
            Self::Month => v.get(5..7).unwrap_or("").to_owned(),
            Self::Day => v.get(8..10).unwrap_or("").to_owned(),
            Self::Trim => v.trim().to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Text(String),
    Digit,
    Letter,
    Alnum,
    Counter(usize),
    Year,
    Column(usize, Vec<Filter>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Template(Vec<Piece>);

impl Template {
    pub fn compile(text: &str, columns: &[String]) -> Self {
        let mut out = Vec::new();
        let mut lit = String::new();
        let mut chars = text.chars().peekable();
        let flush = |lit: &mut String, out: &mut Vec<Piece>| {
            if !lit.is_empty() {
                out.push(Piece::Text(std::mem::take(lit)));
            }
        };
        while let Some(c) = chars.next() {
            let piece = match c {
                '\\' => {
                    if let Some(n) = chars.next() {
                        lit.push(n);
                    }
                    continue;
                }
                '#' => Piece::Digit,
                '?' => Piece::Letter,
                '*' => Piece::Alnum,
                '{' => {
                    let mut inner = String::new();
                    let mut closed = false;
                    for n in chars.by_ref() {
                        if n == '}' {
                            closed = true;
                            break;
                        }
                        inner.push(n);
                    }
                    let Some(p) = token(&inner, columns).filter(|_| closed) else {
                        lit.push('{');
                        lit.push_str(&inner);
                        if closed {
                            lit.push('}');
                        }
                        continue;
                    };
                    p
                }
                c => {
                    lit.push(c);
                    continue;
                }
            };
            flush(&mut lit, &mut out);
            out.push(piece);
        }
        flush(&mut lit, &mut out);
        Self(out)
    }

    pub fn reads(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().filter_map(|p| match p {
            Piece::Column(i, _) => Some(*i),
            _ => None,
        })
    }

    pub fn render(&self, rng: &mut Rng, row: u64, year: i32, values: &[Option<String>]) -> String {
        let mut s = String::new();
        for p in &self.0 {
            match p {
                Piece::Text(t) => s.push_str(t),
                Piece::Digit => s.push(char::from(*rng.pick(DIGITS))),
                Piece::Letter => s.push(char::from(*rng.pick(LETTERS))),
                Piece::Alnum => s.push(char::from(*rng.pick(ALNUM))),
                Piece::Counter(w) => {
                    let _ = write!(s, "{:0>w$}", row + 1, w = *w);
                }
                Piece::Year => s.push_str(&year.to_string()),
                Piece::Column(i, filters) => {
                    let v = values.get(*i).cloned().flatten().unwrap_or_default();
                    s.push_str(&filters.iter().fold(v, |v, f| f.apply(&v)));
                }
            }
        }
        s
    }
}

fn token(inner: &str, columns: &[String]) -> Option<Piece> {
    let t = inner.trim();
    if !t.is_empty() && t.bytes().all(|b| b == b'0') {
        return Some(Piece::Counter(t.len()));
    }
    let mut parts = t.split('|');
    let head = parts.next()?.trim();
    let lower = head.to_ascii_lowercase();
    if matches!(lower.as_str(), "n" | "row" | "riga") {
        return Some(Piece::Counter(1));
    }
    if matches!(lower.as_str(), "year" | "anno") {
        return Some(Piece::Year);
    }
    let at = columns
        .iter()
        .position(|c| c == head)
        .or_else(|| columns.iter().position(|c| c.eq_ignore_ascii_case(head)))?;
    let filters: Option<Vec<Filter>> = parts.map(Filter::parse).collect();
    Some(Piece::Column(at, filters?))
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64),
    Col(usize),
    Neg(Box<Node>),
    Bin(char, Box<Node>, Box<Node>),
    Call(String, Vec<Node>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr(Node);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprError(pub String);

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
    columns: &'a [String],
}

impl Parser<'_> {
    fn skip(&mut self) {
        while self.s.get(self.at).is_some_and(u8::is_ascii_whitespace) {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.s.get(self.at).copied()
    }

    fn fail<T>(&self, what: &str) -> Result<T, ExprError> {
        Err(ExprError(format!("{what} at {}", self.at + 1)))
    }

    fn sum(&mut self) -> Result<Node, ExprError> {
        let mut left = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.at += 1;
            left = Node::Bin(char::from(op), Box::new(left), Box::new(self.product()?));
        }
        Ok(left)
    }

    fn product(&mut self) -> Result<Node, ExprError> {
        let mut left = self.unary()?;
        while let Some(op @ (b'*' | b'/' | b'%')) = self.peek() {
            self.at += 1;
            left = Node::Bin(char::from(op), Box::new(left), Box::new(self.unary()?));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Node, ExprError> {
        if self.peek() == Some(b'-') {
            self.at += 1;
            return Ok(Node::Neg(Box::new(self.unary()?)));
        }
        self.atom()
    }

    fn atom(&mut self) -> Result<Node, ExprError> {
        match self.peek() {
            Some(b'(') => {
                self.at += 1;
                let n = self.sum()?;
                if self.peek() != Some(b')') {
                    return self.fail("missing )");
                }
                self.at += 1;
                Ok(n)
            }
            Some(c) if c.is_ascii_digit() || c == b'.' => {
                let start = self.at;
                while self
                    .s
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
                {
                    self.at += 1;
                }
                let text = String::from_utf8_lossy(&self.s[start..self.at]);
                text.parse()
                    .map(Node::Num)
                    .or_else(|_| self.fail("bad number"))
            }
            Some(b'"' | b'`' | b'[') => {
                let close = match self.s[self.at] {
                    b'[' => b']',
                    q => q,
                };
                self.at += 1;
                let start = self.at;
                while self.s.get(self.at).is_some_and(|c| *c != close) {
                    self.at += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.at]).into_owned();
                self.at += 1;
                self.column(&name)
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 => {
                let start = self.at;
                while self
                    .s
                    .get(self.at)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c >= 0x80)
                {
                    self.at += 1;
                }
                let name = String::from_utf8_lossy(&self.s[start..self.at]).into_owned();
                if self.peek() == Some(b'(') {
                    self.at += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(b')') {
                        loop {
                            args.push(self.sum()?);
                            match self.peek() {
                                Some(b',') => self.at += 1,
                                Some(b')') => break,
                                _ => return self.fail("missing )"),
                            }
                        }
                    }
                    self.at += 1;
                    let f = name.to_ascii_lowercase();
                    let arity = match f.as_str() {
                        "round" => 1..=2,
                        "abs" | "floor" | "ceil" => 1..=1,
                        "min" | "max" => 1..=16,
                        _ => return Err(ExprError(format!("unknown function {name}"))),
                    };
                    if !arity.contains(&args.len()) {
                        return Err(ExprError(format!("wrong arguments for {name}")));
                    }
                    return Ok(Node::Call(f, args));
                }
                self.column(&name)
            }
            Some(_) => self.fail("unexpected character"),
            None => self.fail("missing value"),
        }
    }

    fn column(&self, name: &str) -> Result<Node, ExprError> {
        self.columns
            .iter()
            .position(|c| c == name)
            .or_else(|| {
                self.columns
                    .iter()
                    .position(|c| c.eq_ignore_ascii_case(name))
            })
            .map(Node::Col)
            .ok_or_else(|| ExprError(format!("unknown column {name}")))
    }
}

fn eval(n: &Node, values: &[Option<String>]) -> Option<f64> {
    Some(match n {
        Node::Num(v) => *v,
        Node::Col(i) => values.get(*i)?.as_deref()?.trim().parse().ok()?,
        Node::Neg(a) => -eval(a, values)?,
        Node::Bin(op, a, b) => {
            let (a, b) = (eval(a, values)?, eval(b, values)?);
            match op {
                '+' => a + b,
                '-' => a - b,
                '*' => a * b,
                '/' if b != 0.0 => a / b,
                '%' if b != 0.0 => a % b,
                _ => return None,
            }
        }
        Node::Call(f, args) => {
            let v: Option<Vec<f64>> = args.iter().map(|a| eval(a, values)).collect();
            let v = v?;
            let first = *v.first()?;
            match f.as_str() {
                "abs" => first.abs(),
                "floor" => first.floor(),
                "ceil" => first.ceil(),
                "min" => v.iter().copied().fold(f64::INFINITY, f64::min),
                "max" => v.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                _ => {
                    let digits = v.get(1).copied().unwrap_or(0.0).clamp(0.0, 12.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let k = 10f64.powi(digits as i32);
                    (first * k).round() / k
                }
            }
        }
    })
    .filter(|v| v.is_finite())
}

impl Expr {
    pub fn compile(text: &str, columns: &[String]) -> Result<Self, ExprError> {
        let mut p = Parser {
            s: text.as_bytes(),
            at: 0,
            columns,
        };
        let n = p.sum()?;
        if p.peek().is_some() {
            return p.fail("unexpected text");
        }
        Ok(Self(n))
    }

    pub fn reads(&self) -> Vec<usize> {
        fn walk(n: &Node, out: &mut Vec<usize>) {
            match n {
                Node::Col(i) => out.push(*i),
                Node::Neg(a) => walk(a, out),
                Node::Bin(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
                Node::Num(_) => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.0, &mut out);
        out
    }

    pub fn render(&self, values: &[Option<String>], scale: u32) -> Option<String> {
        let v = eval(&self.0, values)?;
        let s = scale.min(12) as usize;
        let text = format!("{v:.s$}");
        Some(
            if text.starts_with('-') && text[1..].bytes().all(|b| b == b'0' || b == b'.') {
                text[1..].to_owned()
            } else {
                text
            },
        )
    }
}
