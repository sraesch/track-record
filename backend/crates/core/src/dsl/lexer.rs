//! Tokens of the derived metric DSL.

use super::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    /// A plain identifier: letters, digits and `_`, not starting with a digit.
    Ident(String),
    /// A `` `quoted name` `` for metric names with other characters.
    Quoted(String),
    /// A unit annotation of a number, e.g. the `s` in `1.5[s]`.
    Unit(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Dot,
    Eq,
    Eof,
}

impl Token {
    pub fn describe(&self) -> String {
        match self {
            Token::Number(n) => format!("number {n}"),
            Token::Ident(s) => format!("name {s}"),
            Token::Quoted(s) => format!("name `{s}`"),
            Token::Unit(u) => format!("unit [{u}]"),
            Token::Plus => "'+'".into(),
            Token::Minus => "'-'".into(),
            Token::Star => "'*'".into(),
            Token::Slash => "'/'".into(),
            Token::Caret => "'^'".into(),
            Token::LParen => "'('".into(),
            Token::RParen => "')'".into(),
            Token::Comma => "','".into(),
            Token::Dot => "'.'".into(),
            Token::Eq => "'='".into(),
            Token::Eof => "end of expression".into(),
        }
    }
}

pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

pub fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Splits the source into tokens; the last one is always `Eof`.
pub fn tokenize(src: &str) -> Result<Vec<(Token, Span)>, Diagnostic> {
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        let single = match c {
            '+' => Some(Token::Plus),
            '-' => Some(Token::Minus),
            '*' => Some(Token::Star),
            '/' => Some(Token::Slash),
            '^' => Some(Token::Caret),
            '(' => Some(Token::LParen),
            ')' => Some(Token::RParen),
            ',' => Some(Token::Comma),
            '=' => Some(Token::Eq),
            _ => None,
        };
        if let Some(token) = single {
            chars.next();
            out.push((token, Span::new(start, start + 1)));
            continue;
        }
        let next_is_digit = src[start + c.len_utf8()..]
            .chars()
            .next()
            .is_some_and(|d| d.is_ascii_digit());
        if c.is_ascii_digit() || (c == '.' && next_is_digit) {
            let end = number_end(src, start);
            let text = &src[start..end];
            let value: f64 = text.parse().map_err(|_| {
                Diagnostic::new(Span::new(start, end), format!("invalid number {text:?}"))
            })?;
            if !value.is_finite() {
                return Err(Diagnostic::new(
                    Span::new(start, end),
                    "number is too large",
                ));
            }
            while chars.peek().is_some_and(|(i, _)| *i < end) {
                chars.next();
            }
            out.push((Token::Number(value), Span::new(start, end)));
            continue;
        }
        if c == '.' {
            chars.next();
            out.push((Token::Dot, Span::new(start, start + 1)));
            continue;
        }
        if is_ident_start(c) {
            let mut end = start;
            while let Some(&(i, d)) = chars.peek() {
                if !is_ident_char(d) {
                    break;
                }
                end = i + d.len_utf8();
                chars.next();
            }
            out.push((
                Token::Ident(src[start..end].to_owned()),
                Span::new(start, end),
            ));
            continue;
        }
        if c == '`' || c == '[' {
            let close = if c == '`' { '`' } else { ']' };
            chars.next();
            let mut end = None;
            for (i, d) in chars.by_ref() {
                if d == close {
                    end = Some(i);
                    break;
                }
            }
            let Some(end) = end else {
                return Err(Diagnostic::new(
                    Span::new(start, src.len()),
                    format!("missing closing {close:?}"),
                ));
            };
            let raw = &src[start + 1..end];
            let span = Span::new(start, end + 1);
            if c == '`' {
                if raw.is_empty() {
                    return Err(Diagnostic::new(span, "empty quoted name"));
                }
                // Verbatim: a metric name is trimmed where it is read, an
                // attribute name (a parameter key) is not.
                out.push((Token::Quoted(raw.to_owned()), span));
            } else {
                out.push((Token::Unit(raw.trim().to_owned()), span));
            }
            continue;
        }
        return Err(Diagnostic::new(
            Span::new(start, start + c.len_utf8()),
            format!("unexpected character {c:?}"),
        ));
    }
    out.push((Token::Eof, Span::new(src.len(), src.len())));
    Ok(out)
}

/// End of a number literal: digits, an optional fraction and an optional
/// exponent.
fn number_end(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut i = start;
    let digits = |i: &mut usize| {
        while *i < b.len() && b[*i].is_ascii_digit() {
            *i += 1;
        }
    };
    digits(&mut i);
    if i < b.len() && b[i] == b'.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit()) {
        i += 1;
        digits(&mut i);
    } else if i < b.len() && b[i] == b'.' && i > start {
        // "1." is a number followed by a dot only if a name follows; we
        // accept "1." as 1.
        if !b
            .get(i + 1)
            .is_some_and(|d| d.is_ascii_alphabetic() || *d == b'_')
        {
            i += 1;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        if j < b.len() && b[j].is_ascii_digit() {
            i = j;
            digits(&mut i);
        }
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<Token> {
        tokenize(src).unwrap().into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn tokens() {
        assert_eq!(
            kinds("bytes / time_s"),
            vec![
                Token::Ident("bytes".into()),
                Token::Slash,
                Token::Ident("time_s".into()),
                Token::Eof
            ]
        );
        assert_eq!(
            kinds("series.fps"),
            vec![
                Token::Ident("series".into()),
                Token::Dot,
                Token::Ident("fps".into()),
                Token::Eof
            ]
        );
        assert_eq!(
            kinds("1.5e-3 .5 2. 3[ ms ]"),
            vec![
                Token::Number(1.5e-3),
                Token::Number(0.5),
                Token::Number(2.0),
                Token::Number(3.0),
                Token::Unit("ms".into()),
                Token::Eof
            ]
        );
        assert_eq!(
            kinds("`load-time (cold)`"),
            vec![Token::Quoted("load-time (cold)".into()), Token::Eof]
        );
    }

    #[test]
    fn errors_have_spans() {
        let err = tokenize("a $ b").unwrap_err();
        assert_eq!(err.span, Span::new(2, 3));
        let err = tokenize("`open").unwrap_err();
        assert!(err.message.contains("missing closing"));
        assert!(tokenize("``").is_err());
        assert!(tokenize("1e999").is_err());
    }
}
