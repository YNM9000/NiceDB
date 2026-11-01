#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    Quoted(String),
    Str(String),
    Num(f64),
    Op(&'static str),
    Open,
    Close,
    Comma,
}

const OPS: &[&str] = &[
    "<>", "!=", "<=", ">=", "||", "=", "<", ">", "+", "-", "*", "/", "%",
];
const TYPE_TAIL: &[&str] = &["varying", "precision", "without", "with", "time", "zone"];

#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
fn lex(src: &str) -> Option<Vec<Tok>> {
    let s: Vec<char> = src.replace("\\'", "'").chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    while i < s.len() {
        let c = s[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '(' {
            out.push(Tok::Open);
            i += 1;
        } else if c == ')' {
            out.push(Tok::Close);
            i += 1;
        } else if c == ',' {
            out.push(Tok::Comma);
            i += 1;
        } else if c == '\'' {
            let mut v = String::new();
            i += 1;
            loop {
                let ch = *s.get(i)?;
                i += 1;
                if ch == '\'' {
                    if s.get(i) == Some(&'\'') {
                        v.push('\'');
                        i += 1;
                        continue;
                    }
                    break;
                }
                v.push(ch);
            }
            out.push(Tok::Str(v));
        } else if c == '"' || c == '`' || c == '[' {
            let close = if c == '[' { ']' } else { c };
            let end = s[i + 1..].iter().position(|&x| x == close)? + i + 1;
            out.push(Tok::Quoted(
                s[i + 1..end].iter().collect::<String>().to_lowercase(),
            ));
            i = end + 1;
        } else if c == ':' {
            while s.get(i) == Some(&':') {
                i += 1;
            }
            loop {
                while s.get(i).is_some_and(|c| c.is_whitespace()) {
                    i += 1;
                }
                let start = i;
                while s.get(i).is_some_and(|&c| word_char(c)) {
                    i += 1;
                }
                if start == i {
                    break;
                }
                let mut j = i;
                while s.get(j).is_some_and(|c| c.is_whitespace()) {
                    j += 1;
                }
                let k = j;
                while s.get(j).is_some_and(|&c| word_char(c)) {
                    j += 1;
                }
                let next: String = s[k..j].iter().collect::<String>().to_lowercase();
                if !TYPE_TAIL.contains(&next.as_str()) {
                    break;
                }
            }
            if s.get(i) == Some(&'(') {
                let end = s[i..].iter().position(|&x| x == ')')? + i;
                i = end + 1;
            }
            if s.get(i) == Some(&'[') && s.get(i + 1) == Some(&']') {
                i += 2;
            }
        } else if c.is_ascii_digit() || (c == '.' && s.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let start = i;
            while s.get(i).is_some_and(|c| c.is_ascii_digit() || *c == '.') {
                i += 1;
            }
            if s.get(i).is_some_and(|c| *c == 'e' || *c == 'E')
                && s.get(i + 1)
                    .is_some_and(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
            {
                i += 2;
                while s.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            out.push(Tok::Num(
                s[start..i].iter().collect::<String>().parse().ok()?,
            ));
        } else if word_char(c) {
            let start = i;
            while s.get(i).is_some_and(|&c| word_char(c)) {
                i += 1;
            }
            let w: String = s[start..i].iter().collect::<String>().to_lowercase();
            if s.get(i) == Some(&'\'') && (w.starts_with('_') || w == "n" || w == "e") {
                continue;
            }
            out.push(Tok::Name(w));
        } else {
            let rest: String = s[i..s.len().min(i + 2)].iter().collect();
            let op = OPS.iter().find(|o| rest.starts_with(*o))?;
            out.push(Tok::Op(if op == &"!=" { "<>" } else { op }));
            i += op.len();
        }
    }
    Some(out)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Length,
    Lower,
    Upper,
    Trim,
    Abs,
    Coalesce,
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Col(usize),
    Num(f64),
    Str(String),
    Null,
    Bool(bool),
    Not(Box<Node>),
    And(Box<Node>, Box<Node>),
    Or(Box<Node>, Box<Node>),
    Cmp(&'static str, Box<Node>, Box<Node>),
    Math(&'static str, Box<Node>, Box<Node>),
    Neg(Box<Node>),
    In(Box<Node>, Vec<Node>, bool),
    Between(Box<Node>, Box<Node>, Box<Node>, bool),
    IsNull(Box<Node>, bool),
    Like(Box<Node>, Box<Node>, bool),
    Call(Func, Vec<Node>),
}

struct Parser<'a> {
    t: Vec<Tok>,
    i: usize,
    names: &'a [String],
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Name(x)) if x == w)
    }

    fn eat_word(&mut self, w: &str) -> bool {
        let yes = self.word(w);
        self.i += usize::from(yes);
        yes
    }

    fn eat(&mut self, t: &Tok) -> bool {
        let yes = self.peek() == Some(t);
        self.i += usize::from(yes);
        yes
    }

    fn or(&mut self) -> Option<Node> {
        let mut l = self.and()?;
        while self.eat_word("or") {
            l = Node::Or(Box::new(l), Box::new(self.and()?));
        }
        Some(l)
    }

    fn and(&mut self) -> Option<Node> {
        let mut l = self.not()?;
        while self.eat_word("and") {
            l = Node::And(Box::new(l), Box::new(self.not()?));
        }
        Some(l)
    }

    fn not(&mut self) -> Option<Node> {
        if self.eat_word("not") {
            return Some(Node::Not(Box::new(self.not()?)));
        }
        self.predicate()
    }

    fn predicate(&mut self) -> Option<Node> {
        let l = self.sum()?;
        if self.eat_word("is") {
            let neg = self.eat_word("not");
            return self
                .eat_word("null")
                .then(|| Node::IsNull(Box::new(l), neg));
        }
        let neg = self.eat_word("not");
        if self.eat_word("in") {
            if !self.eat(&Tok::Open) {
                return None;
            }
            let mut list = vec![self.sum()?];
            while self.eat(&Tok::Comma) {
                list.push(self.sum()?);
            }
            return self
                .eat(&Tok::Close)
                .then(|| Node::In(Box::new(l), list, neg));
        }
        if self.eat_word("between") {
            let lo = self.sum()?;
            if !self.eat_word("and") {
                return None;
            }
            let hi = self.sum()?;
            return Some(Node::Between(Box::new(l), Box::new(lo), Box::new(hi), neg));
        }
        if self.eat_word("like") {
            return Some(Node::Like(Box::new(l), Box::new(self.sum()?), neg));
        }
        if neg {
            return None;
        }
        if let Some(Tok::Op(op @ ("=" | "<>" | "<" | "<=" | ">" | ">="))) = self.peek().cloned() {
            self.i += 1;
            let r = self.sum()?;
            return Some(Node::Cmp(op, Box::new(l), Box::new(r)));
        }
        Some(l)
    }

    fn sum(&mut self) -> Option<Node> {
        let mut l = self.product()?;
        while let Some(Tok::Op(op @ ("+" | "-" | "||"))) = self.peek().cloned() {
            self.i += 1;
            l = Node::Math(op, Box::new(l), Box::new(self.product()?));
        }
        Some(l)
    }

    fn product(&mut self) -> Option<Node> {
        let mut l = self.unary()?;
        while let Some(Tok::Op(op @ ("*" | "/" | "%"))) = self.peek().cloned() {
            self.i += 1;
            l = Node::Math(op, Box::new(l), Box::new(self.unary()?));
        }
        Some(l)
    }

    fn unary(&mut self) -> Option<Node> {
        if self.eat(&Tok::Op("-")) {
            return Some(Node::Neg(Box::new(self.unary()?)));
        }
        self.primary()
    }

    fn column(&self, name: &str) -> Option<Node> {
        self.names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .map(Node::Col)
    }

    fn primary(&mut self) -> Option<Node> {
        let t = self.peek()?.clone();
        self.i += 1;
        match t {
            Tok::Num(n) => Some(Node::Num(n)),
            Tok::Str(s) => Some(Node::Str(s)),
            Tok::Quoted(n) => self.column(&n),
            Tok::Open => {
                let e = self.or()?;
                self.eat(&Tok::Close).then_some(e)
            }
            Tok::Name(w) => match w.as_str() {
                "null" => Some(Node::Null),
                "true" => Some(Node::Bool(true)),
                "false" => Some(Node::Bool(false)),
                "cast" if self.peek() == Some(&Tok::Open) => {
                    self.i += 1;
                    let e = self.or()?;
                    if !self.eat_word("as") {
                        return None;
                    }
                    let mut depth = 0;
                    while let Some(t) = self.peek() {
                        match t {
                            Tok::Open => depth += 1,
                            Tok::Close if depth == 0 => break,
                            Tok::Close => depth -= 1,
                            _ => {}
                        }
                        self.i += 1;
                    }
                    self.eat(&Tok::Close).then_some(e)
                }
                _ if self.peek() == Some(&Tok::Open) => {
                    let f = match w.as_str() {
                        "length" | "char_length" | "character_length" | "len" => Func::Length,
                        "lower" | "lcase" => Func::Lower,
                        "upper" | "ucase" => Func::Upper,
                        "trim" | "btrim" => Func::Trim,
                        "abs" => Func::Abs,
                        "coalesce" | "ifnull" | "isnull" => Func::Coalesce,
                        _ => return None,
                    };
                    self.i += 1;
                    let mut args = vec![self.or()?];
                    while self.eat(&Tok::Comma) {
                        args.push(self.or()?);
                    }
                    self.eat(&Tok::Close).then_some(Node::Call(f, args))
                }
                _ => self.column(&w),
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Null,
    Num(f64),
    Str(String),
    Bool(bool),
}

impl Val {
    fn num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            Val::Str(s) => s.trim().parse().ok(),
            Val::Bool(b) => Some(f64::from(u8::from(*b))),
            Val::Null => None,
        }
    }

    fn text(&self) -> Option<String> {
        match self {
            Val::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => Some(format!("{n:.0}")),
            Val::Num(n) => Some(n.to_string()),
            Val::Str(s) => Some(s.clone()),
            Val::Bool(b) => Some(b.to_string()),
            Val::Null => None,
        }
    }

    fn truth(&self) -> Option<bool> {
        match self {
            Val::Bool(b) => Some(*b),
            Val::Null => None,
            Val::Num(n) => Some(*n != 0.0),
            Val::Str(s) => match s.to_ascii_lowercase().as_str() {
                "t" | "true" | "1" | "y" | "yes" => Some(true),
                "f" | "false" | "0" | "n" | "no" => Some(false),
                _ => None,
            },
        }
    }
}

#[allow(clippy::many_single_char_names)]
fn order(a: &Val, b: &Val) -> Option<std::cmp::Ordering> {
    if matches!(a, Val::Null) || matches!(b, Val::Null) {
        return None;
    }
    match (a.num(), b.num()) {
        (Some(x), Some(y)) if !matches!((a, b), (Val::Str(_), Val::Str(_))) => x.partial_cmp(&y),
        _ => {
            let (x, y) = (a.text()?, b.text()?);
            match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                (Ok(p), Ok(q)) => p.partial_cmp(&q),
                _ => Some(x.cmp(&y)),
            }
        }
    }
}

fn like(text: &[char], pat: &[char]) -> bool {
    match pat.split_first() {
        None => text.is_empty(),
        Some(('%', rest)) => (0..=text.len()).any(|k| like(&text[k..], rest)),
        Some(('_', rest)) => !text.is_empty() && like(&text[1..], rest),
        Some((c, rest)) => text.first() == Some(c) && like(&text[1..], rest),
    }
}

fn bool3(b: Option<bool>) -> Val {
    b.map_or(Val::Null, Val::Bool)
}

#[allow(clippy::many_single_char_names)]
fn eval(n: &Node, row: &[Option<String>]) -> Val {
    match n {
        Node::Col(i) => row.get(*i).cloned().flatten().map_or(Val::Null, Val::Str),
        Node::Num(x) => Val::Num(*x),
        Node::Str(s) => Val::Str(s.clone()),
        Node::Null => Val::Null,
        Node::Bool(b) => Val::Bool(*b),
        Node::Not(e) => bool3(eval(e, row).truth().map(|b| !b)),
        Node::And(a, b) => match (eval(a, row).truth(), eval(b, row).truth()) {
            (Some(false), _) | (_, Some(false)) => Val::Bool(false),
            (Some(true), Some(true)) => Val::Bool(true),
            _ => Val::Null,
        },
        Node::Or(a, b) => match (eval(a, row).truth(), eval(b, row).truth()) {
            (Some(true), _) | (_, Some(true)) => Val::Bool(true),
            (Some(false), Some(false)) => Val::Bool(false),
            _ => Val::Null,
        },
        Node::Cmp(op, a, b) => {
            let o = order(&eval(a, row), &eval(b, row));
            bool3(o.map(|o| match *op {
                "=" => o.is_eq(),
                "<>" => o.is_ne(),
                "<" => o.is_lt(),
                "<=" => o.is_le(),
                ">" => o.is_gt(),
                _ => o.is_ge(),
            }))
        }
        Node::Math("||", a, b) => match (eval(a, row).text(), eval(b, row).text()) {
            (Some(x), Some(y)) => Val::Str(x + &y),
            _ => Val::Null,
        },
        Node::Math(op, a, b) => match (eval(a, row).num(), eval(b, row).num()) {
            (Some(x), Some(y)) => match *op {
                "+" => Val::Num(x + y),
                "-" => Val::Num(x - y),
                "*" => Val::Num(x * y),
                "/" if y != 0.0 => Val::Num(x / y),
                "%" if y != 0.0 => Val::Num(x % y),
                _ => Val::Null,
            },
            _ => Val::Null,
        },
        Node::Neg(e) => eval(e, row).num().map_or(Val::Null, |x| Val::Num(-x)),
        Node::In(e, list, neg) => {
            let v = eval(e, row);
            if matches!(v, Val::Null) {
                return Val::Null;
            }
            let hit = list
                .iter()
                .any(|x| order(&v, &eval(x, row)).is_some_and(std::cmp::Ordering::is_eq));
            Val::Bool(hit != *neg)
        }
        Node::Between(e, lo, hi, neg) => {
            let v = eval(e, row);
            let a = order(&v, &eval(lo, row)).map(std::cmp::Ordering::is_ge);
            let b = order(&v, &eval(hi, row)).map(std::cmp::Ordering::is_le);
            match (a, b) {
                (Some(x), Some(y)) => Val::Bool((x && y) != *neg),
                _ => Val::Null,
            }
        }
        Node::IsNull(e, neg) => Val::Bool(matches!(eval(e, row), Val::Null) != *neg),
        Node::Like(e, p, neg) => match (eval(e, row).text(), eval(p, row).text()) {
            (Some(t), Some(p)) => {
                let (t, p): (Vec<char>, Vec<char>) = (t.chars().collect(), p.chars().collect());
                Val::Bool(like(&t, &p) != *neg)
            }
            _ => Val::Null,
        },
        Node::Call(f, args) => {
            let first = args.first().map_or(Val::Null, |a| eval(a, row));
            match f {
                Func::Coalesce => args
                    .iter()
                    .map(|a| eval(a, row))
                    .find(|v| !matches!(v, Val::Null))
                    .unwrap_or(Val::Null),
                #[allow(clippy::cast_precision_loss)]
                Func::Length => first
                    .text()
                    .map_or(Val::Null, |s| Val::Num(s.chars().count() as f64)),
                Func::Lower => first
                    .text()
                    .map_or(Val::Null, |s| Val::Str(s.to_lowercase())),
                Func::Upper => first
                    .text()
                    .map_or(Val::Null, |s| Val::Str(s.to_uppercase())),
                Func::Trim => first
                    .text()
                    .map_or(Val::Null, |s| Val::Str(s.trim().to_owned())),
                Func::Abs => first.num().map_or(Val::Null, |x| Val::Num(x.abs())),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub text: String,
    node: Node,
}

impl Rule {
    pub fn compile(expr: &str, names: &[String]) -> Option<Rule> {
        let mut t = lex(expr)?;
        if matches!(t.first(), Some(Tok::Name(w)) if w == "check") {
            t.remove(0);
        }
        let mut p = Parser { t, i: 0, names };
        let node = p.or()?;
        (p.i == p.t.len()).then(|| Rule {
            text: expr.trim().to_owned(),
            node,
        })
    }

    pub fn holds(&self, row: &[Option<String>]) -> bool {
        eval(&self.node, row).truth() != Some(false)
    }
}
