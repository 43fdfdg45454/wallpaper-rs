use serde_json::{Value as Json, json};

pub(super) const MAX_LEN: usize = 256;
const MAX_DEPTH: usize = 8;
const OPS: [&str; 11] = ["==", "!=", "<=", ">=", "&&", "||", "<", ">", "!", "(", ")"];
pub(super) const CMP: [&str; 6] = ["==", "!=", "<=", ">=", "<", ">"];

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Id(String),
    Lit(Json),
    Not(Box<Expr>),
    Both(Box<Expr>, Box<Expr>),
    Either(Box<Expr>, Box<Expr>),
    Cmp(Box<Expr>, &'static str, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Token {
    Id(String),
    Lit(Json),
    Op(&'static str),
}

pub(super) struct Parser {
    tokens: Vec<Token>,
    at: usize,
    depth: usize,
}

pub(super) fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let (mut out, mut rest) = (Vec::new(), text.trim_start());
    while let Some(c) = rest.chars().next() {
        let word = |ok: fn(char) -> bool| rest[1..].find(|c: char| !ok(c)).map_or(rest.len(), |n| n + 1);
        let (token, len) = if let Some(op) = OPS.iter().find(|o| rest.starts_with(**o)) {
            (Token::Op(op), op.len())
        } else if c == '\'' || c == '"' {
            let end = rest[1..].find(c).ok_or("a text has no closing quote")?;
            (Token::Lit(json!(&rest[1..=end])), end + 2)
        } else if c.is_ascii_digit() || c == '-' {
            let len = word(|c| c.is_ascii_digit() || c == '.');
            let n: f64 = rest[..len].parse().map_err(|_| format!("{} is not a number", &rest[..len]))?;
            (Token::Lit(json!(n)), len)
        } else if c.is_ascii_alphabetic() {
            let len = word(|c| c.is_ascii_alphanumeric() || c == '_');
            let token = match &rest[..len] {
                "true" | "false" => Token::Lit(json!(&rest[..len] == "true")),
                id => Token::Id(id.into()),
            };
            (token, len)
        } else {
            return Err(format!("unexpected {c:?}"));
        };
        out.push(token);
        rest = rest[len..].trim_start();
    }
    Ok(out)
}

pub fn rename(text: &str, from: &str, to: &str) -> String {
    let (mut out, mut rest) = (String::new(), text);
    while let Some(c) = rest.chars().next() {
        let len = match c {
            '\'' | '"' => rest[1..].find(c).map_or(rest.len(), |n| n + 2),
            _ if c.is_ascii_alphanumeric() => {
                rest.find(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '.').unwrap_or(rest.len())
            }
            _ => c.len_utf8(),
        };
        let word = &rest[..len];
        out.push_str(if word == from && c.is_ascii_alphabetic() && !["true", "false"].contains(&word) {
            to
        } else {
            word
        });
        rest = &rest[len..];
    }
    out
}

impl Parser {
    pub(super) fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens, at: 0, depth: 0 }
    }

    pub(super) fn done(&self) -> bool {
        self.at == self.tokens.len()
    }

    fn eat(&mut self, op: &str) -> bool {
        let found = matches!(self.tokens.get(self.at), Some(Token::Op(o)) if *o == op);
        self.at += usize::from(found);
        found
    }

    pub(super) fn either(&mut self) -> Result<Expr, String> {
        let mut left = self.both()?;
        while self.eat("||") {
            left = Expr::Either(Box::new(left), Box::new(self.both()?));
        }
        Ok(left)
    }

    fn both(&mut self) -> Result<Expr, String> {
        let mut left = self.compare()?;
        while self.eat("&&") {
            left = Expr::Both(Box::new(left), Box::new(self.compare()?));
        }
        Ok(left)
    }

    fn compare(&mut self) -> Result<Expr, String> {
        let left = self.unary()?;
        match CMP.iter().find(|op| self.eat(op)) {
            Some(op) => Ok(Expr::Cmp(Box::new(left), op, Box::new(self.unary()?))),
            None => Ok(left),
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(format!("is nested deeper than {MAX_DEPTH} levels"));
        }
        let found = if self.eat("!") { Expr::Not(Box::new(self.unary()?)) } else { self.atom()? };
        self.depth -= 1;
        Ok(found)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        let token = self.tokens.get(self.at).cloned().ok_or("ends too early")?;
        self.at += 1;
        match token {
            Token::Id(id) => Ok(Expr::Id(id)),
            Token::Lit(value) => Ok(Expr::Lit(value)),
            Token::Op("(") => {
                let inner = self.either()?;
                self.eat(")").then_some(inner).ok_or_else(|| "misses a )".into())
            }
            Token::Op(op) => Err(format!("has {op} where a value belongs")),
        }
    }
}
