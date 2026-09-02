//! Pratt parser for the Base expression language.

use anyhow::{Result, bail};

use super::lexer::{Token, lex};
use super::{ArithOp, CmpOp, Expr, FileField, LogicOp, PropertyRef};

pub(super) const MAX_EXPRESSION_DEPTH: u32 = 128;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Parser {
    tokens: Vec<Token>,
    position: usize,
    depth: u32,
}

impl Parser {
    pub(super) const fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
            depth: 0,
        }
    }

    pub(super) const fn at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn peek_offset(&self, offset: usize) -> Option<&Token> {
        let index = self.position.checked_add(offset)?;
        self.tokens.get(index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        if token.is_some() {
            self.position = self.position.saturating_add(1);
        }
        token
    }

    fn expect(&mut self, token: &Token) -> Result<()> {
        if self.next().as_ref() == Some(token) {
            Ok(())
        } else {
            bail!("expected {token:?} in expression")
        }
    }

    pub(super) fn parse_expression(&mut self) -> Result<Expr> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > MAX_EXPRESSION_DEPTH {
            bail!("expression nests too deeply (limit {MAX_EXPRESSION_DEPTH})");
        }
        let expression = self.parse_logic();
        self.depth = self.depth.saturating_sub(1);
        expression
    }

    fn parse_logic(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Some(Token::And) => LogicOp::And,
                Some(Token::Or) => LogicOp::Or,
                _ => break,
            };
            self.next();
            let right = self.parse_comparison()?;
            left = Expr::Logic {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let left = self.parse_additive()?;
        let op = match self.peek() {
            Some(Token::Equal) => CmpOp::Equal,
            Some(Token::NotEqual) => CmpOp::NotEqual,
            Some(Token::Greater) => CmpOp::Greater,
            Some(Token::GreaterEqual) => CmpOp::GreaterEqual,
            Some(Token::Less) => CmpOp::Less,
            Some(Token::LessEqual) => CmpOp::LessEqual,
            _ => return Ok(left),
        };
        self.next();
        let right = self.parse_additive()?;
        Ok(Expr::Compare {
            op,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_additive(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => ArithOp::Add,
                Some(Token::Minus) => ArithOp::Subtract,
                _ => break,
            };
            self.next();
            let right = self.parse_multiplicative()?;
            left = Expr::Arithmetic {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => ArithOp::Multiply,
                Some(Token::Slash) => ArithOp::Divide,
                Some(Token::Percent) => ArithOp::Modulo,
                _ => break,
            };
            self.next();
            let right = self.parse_unary()?;
            left = Expr::Arithmetic {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        let mut operators = Vec::new();
        while let Some(operator @ (Token::Bang | Token::Minus)) = self.peek().cloned() {
            self.next();
            operators.push(operator);
        }
        self.depth = self
            .depth
            .saturating_add(u32::try_from(operators.len()).unwrap_or(u32::MAX));
        if self.depth > MAX_EXPRESSION_DEPTH {
            bail!("expression nests too deeply (limit {MAX_EXPRESSION_DEPTH})");
        }
        let wrapped = self.parse_postfix();
        self.depth = self
            .depth
            .saturating_sub(u32::try_from(operators.len()).unwrap_or(u32::MAX));
        let mut expression = wrapped?;
        while let Some(operator) = operators.pop() {
            expression = match operator {
                Token::Bang => Expr::Not(Box::new(expression)),
                Token::Minus => Expr::Neg(Box::new(expression)),
                _ => break,
            };
        }
        Ok(expression)
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut subject = self.parse_primary()?;
        while self.peek() == Some(&Token::Dot) {
            // A dot followed by ident + '(' is a method call; otherwise it
            // extends the property chain consumed by parse_primary.
            let method_candidate = matches!(self.peek_offset(1), Some(Token::Ident(_)))
                && matches!(self.peek_offset(2), Some(Token::LParen));
            if !method_candidate {
                break;
            }
            self.next();
            let Some(Token::Ident(name)) = self.next() else {
                bail!("expected method name");
            };
            self.expect(&Token::LParen)?;
            let args = self.parse_arguments()?;
            subject = Expr::Method {
                subject: Box::new(subject),
                name,
                args,
            };
        }
        Ok(subject)
    }

    fn parse_arguments(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();
        if self.peek() == Some(&Token::RParen) {
            self.next();
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression()?);
            match self.next() {
                Some(Token::RParen) => break,
                Some(Token::Comma) => {}
                _ => bail!("expected ',' or ')' in argument list"),
            }
        }
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.next() {
            Some(Token::Number(value)) => Ok(Expr::Number(value)),
            Some(Token::Text(text)) => Ok(Expr::Text(text)),
            Some(Token::LParen) => {
                let inner = self.parse_expression()?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(Token::Ident(first)) => self.parse_chain(first),
            None => bail!("unexpected end of expression"),
            other => bail!("unexpected {other:?} in expression"),
        }
    }

    /// Parses a dotted/bracketed identifier chain starting with `first`,
    /// resolving it to a property reference.
    fn parse_chain(&mut self, first: String) -> Result<Expr> {
        let mut segments = vec![first];
        loop {
            match self.peek() {
                Some(Token::Dot) => {
                    if matches!(self.peek_offset(1), Some(Token::Ident(_)))
                        && matches!(self.peek_offset(2), Some(Token::LParen))
                    {
                        break;
                    }
                    self.next();
                    match self.next() {
                        Some(Token::Ident(next)) => segments.push(next),
                        _ => bail!("expected a property name after '.'"),
                    }
                }
                Some(Token::LBracket) => {
                    self.next();
                    let key = match self.next() {
                        Some(Token::Text(key)) => key,
                        Some(Token::Number(value)) => format_number(value),
                        _ => bail!("expected a string or number property key"),
                    };
                    self.expect(&Token::RBracket)?;
                    segments.push(key);
                    // Consume an optional dot so `a["b"].c` keeps chaining,
                    // unless that dot starts a method call.
                    let method_follows = matches!(self.peek_offset(1), Some(Token::Ident(_)))
                        && matches!(self.peek_offset(2), Some(Token::LParen));
                    if self.peek() == Some(&Token::Dot) && !method_follows {
                        self.next();
                        match self.next() {
                            Some(Token::Ident(next)) => segments.push(next),
                            _ => bail!("expected a property name after '.'"),
                        }
                    }
                }
                _ => break,
            }
        }
        // Bare global function calls such as `today()` or `if(...)`.
        if self.peek() == Some(&Token::LParen) && segments.len() == 1 {
            self.next();
            let args = self.parse_arguments()?;
            return Ok(Expr::Call(segments.remove(0), args));
        }
        // Keyword literals.
        if let [only] = segments.as_slice() {
            match only.as_str() {
                "null" => return Ok(Expr::Null),
                "true" => return Ok(Expr::Bool(true)),
                "false" => return Ok(Expr::Bool(false)),
                _ => {}
            }
        }
        // Function-style predicates written as `file.hasTag(...)`.
        if self.peek() == Some(&Token::Dot)
            && let Some(Token::Ident(member)) = self.peek_offset(1)
            && matches!(self.peek_offset(2), Some(Token::LParen))
        {
            let member = member.clone();
            if segments.as_slice() == ["file"]
                && matches!(member.as_str(), "hasTag" | "hasLink" | "inFolder")
            {
                self.next();
                self.next();
                self.expect(&Token::LParen)?;
                let args = self.parse_arguments()?;
                return Ok(Expr::Call(format!("file.{member}"), args));
            }
        }
        resolve_chain(&segments)
    }
}

fn resolve_chain(segments: &[String]) -> Result<Expr> {
    let Some(head) = segments.first() else {
        bail!("empty property reference");
    };
    if head == "file" {
        let Some(field) = segments.get(1) else {
            bail!("file reference must name a file property");
        };
        if segments.len() > 2 {
            let extra = segments
                .iter()
                .skip(1)
                .cloned()
                .collect::<Vec<_>>()
                .join(".");
            bail!("unknown file property {extra:?}");
        }
        let field = parse_file_field(field)?;
        return Ok(Expr::Property(PropertyRef::File(field)));
    }
    if head == "formula" {
        if segments.len() < 2 {
            bail!("formula reference must name a declared formula");
        }
        let name = segments
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>()
            .join(".");
        return Ok(Expr::Property(PropertyRef::Formula(name)));
    }
    let note_segments: Vec<String> = if head == "note" {
        segments.iter().skip(1).cloned().collect()
    } else {
        segments.to_vec()
    };
    if note_segments.is_empty() {
        bail!("property reference must not be empty");
    }
    Ok(Expr::Property(PropertyRef::Note(note_segments)))
}

/// Parses a standalone property source such as `file.mtime` or `note["a"]`.
///
/// # Errors
/// Fails when the source is not a supported property reference.
pub fn parse_property_ref(source: &str) -> Result<PropertyRef> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    let parsed = parser.parse_expression()?;
    if !parser.at_end() {
        bail!("unexpected trailing input in property {source:?}");
    }
    match parsed {
        Expr::Property(reference) => Ok(reference),
        _ => bail!("{source:?} is not a property reference"),
    }
}

fn parse_file_field(field: &str) -> Result<FileField> {
    match field {
        "name" => Ok(FileField::Name),
        "ext" => Ok(FileField::Ext),
        "path" => Ok(FileField::Path),
        "folder" => Ok(FileField::Folder),
        "size" => Ok(FileField::Size),
        "mtime" => Ok(FileField::Mtime),
        "ctime" => Ok(FileField::Ctime),
        "links" => Ok(FileField::Links),
        "tags" => Ok(FileField::Tags),
        "embeds" => Ok(FileField::Embeds),
        "backlinks" => Ok(FileField::Backlinks),
        "properties" => Ok(FileField::Properties),
        _ => bail!("unknown file property {field:?}"),
    }
}

// The range check guarantees the value fits an i64 before formatting.
fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}
