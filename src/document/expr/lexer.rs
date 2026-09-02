//! Tokenizer for the Base expression language.

use anyhow::{Result, bail};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Token {
    Number(f64),
    Text(String),
    Ident(String),
    LParen,
    RParen,
    Comma,
    Dot,
    LBracket,
    RBracket,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    And,
    Or,
    Bang,
}

pub(super) fn lex(source: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some(&(index, character)) = chars.peek() {
        match character {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '\'' | '"' => {
                let text = lex_text(index, character, &mut chars)?;
                tokens.push(Token::Text(text));
            }
            '0'..='9' => {
                let number = lex_number(source, index, &mut chars)?;
                tokens.push(Token::Number(number));
            }
            '(' => push_simple(&mut chars, &mut tokens, Token::LParen),
            ')' => push_simple(&mut chars, &mut tokens, Token::RParen),
            ',' => push_simple(&mut chars, &mut tokens, Token::Comma),
            '.' => push_simple(&mut chars, &mut tokens, Token::Dot),
            '[' => push_simple(&mut chars, &mut tokens, Token::LBracket),
            ']' => push_simple(&mut chars, &mut tokens, Token::RBracket),
            '+' => push_simple(&mut chars, &mut tokens, Token::Plus),
            '-' => push_simple(&mut chars, &mut tokens, Token::Minus),
            '*' => push_simple(&mut chars, &mut tokens, Token::Star),
            '/' => push_simple(&mut chars, &mut tokens, Token::Slash),
            '%' => push_simple(&mut chars, &mut tokens, Token::Percent),
            '>' => push_operator(
                &mut chars,
                &mut tokens,
                Token::Greater,
                '=',
                Token::GreaterEqual,
            ),
            '<' => push_operator(&mut chars, &mut tokens, Token::Less, '=', Token::LessEqual),
            '=' => push_operator(&mut chars, &mut tokens, Token::Equal, '=', Token::Equal),
            '!' => push_operator(&mut chars, &mut tokens, Token::Bang, '=', Token::NotEqual),
            '&' => push_doubled(&mut chars, &mut tokens, '&', Token::And)?,
            '|' => push_doubled(&mut chars, &mut tokens, '|', Token::Or)?,
            _ if character.is_alphabetic() || character == '_' => {
                let ident = lex_ident(source, index, &mut chars);
                tokens.push(Token::Ident(ident));
            }
            _ => bail!("unexpected character {character:?} in expression"),
        }
    }
    Ok(tokens)
}

fn advance(chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>) {
    chars.next();
}

fn push_simple(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    tokens: &mut Vec<Token>,
    token: Token,
) {
    advance(chars);
    tokens.push(token);
}

fn push_operator(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    tokens: &mut Vec<Token>,
    single: Token,
    doubled_suffix: char,
    doubled: Token,
) {
    advance(chars);
    if chars.peek().is_some_and(|&(_, c)| c == doubled_suffix) {
        advance(chars);
        tokens.push(doubled);
    } else {
        tokens.push(single);
    }
}

fn push_doubled(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    tokens: &mut Vec<Token>,
    expected: char,
    token: Token,
) -> Result<()> {
    advance(chars);
    if chars.peek().is_some_and(|&(_, c)| c == expected) {
        advance(chars);
        tokens.push(token);
        Ok(())
    } else {
        bail!("expected {expected}{expected} operator");
    }
}

fn lex_text(
    start: usize,
    quote: char,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<String> {
    advance(chars);
    let mut text = String::new();
    while let Some(&(_, character)) = chars.peek() {
        match character {
            c if c == quote => {
                advance(chars);
                return Ok(text);
            }
            '\\' => {
                advance(chars);
                let escaped = chars.peek().map(|&(_, c)| c);
                match escaped {
                    Some('\\' | '\'' | '"') => {
                        if let Some(&(_, c)) = chars.peek() {
                            text.push(c);
                            advance(chars);
                        }
                    }
                    Some('n') => {
                        advance(chars);
                        text.push('\n');
                    }
                    Some('t') => {
                        advance(chars);
                        text.push('\t');
                    }
                    other => bail!(
                        "unsupported escape {:?} in string at offset {start}",
                        other.map(|c| c.to_string())
                    ),
                }
            }
            _ => {
                text.push(character);
                advance(chars);
            }
        }
    }
    bail!("unterminated string literal")
}

fn lex_number(
    source: &str,
    start: usize,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> Result<f64> {
    let mut end = start;
    while let Some(&(index, character)) = chars.peek() {
        if character.is_ascii_digit() || character == '.' {
            end = index.saturating_add(character.len_utf8());
            advance(chars);
        } else {
            break;
        }
    }
    let text = source
        .get(start..end)
        .ok_or_else(|| anyhow::anyhow!("invalid number literal"))?;
    text.parse::<f64>()
        .map_err(|_| anyhow::anyhow!("invalid number literal {text:?}"))
}

fn lex_ident(
    source: &str,
    start: usize,
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
) -> String {
    let mut end = start;
    while let Some(&(index, character)) = chars.peek() {
        if character.is_alphanumeric() || character == '_' {
            end = index.saturating_add(character.len_utf8());
            advance(chars);
        } else {
            break;
        }
    }
    source.get(start..end).unwrap_or_default().to_string()
}
