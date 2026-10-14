//! PDF Lexer / Tokenizer conforming to ISO 32000-1 Section 7.2.
//!
//! Traceability: Section 7.1 PDF Internal Structure, DR-01, NFR-04.2.

use crate::error::{PdfError, Result};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Keyword(String),
    Integer(i64),
    Real(f64),
    StringLiteral(Vec<u8>),
    HexString(Vec<u8>),
    Name(String),
    ArrayOpen,     // [
    ArrayClose,    // ]
    DictOpen,      // <<
    DictClose,     // >>
    Eof,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Keyword(kw) => write!(f, "{}", kw),
            Token::Integer(i) => write!(f, "{}", i),
            Token::Real(r) => write!(f, "{}", r),
            Token::StringLiteral(_) => write!(f, "(...)"),
            Token::HexString(_) => write!(f, "<...>"),
            Token::Name(n) => write!(f, "/{}", n),
            Token::ArrayOpen => write!(f, "["),
            Token::ArrayClose => write!(f, "]"),
            Token::DictOpen => write!(f, "<<"),
            Token::DictClose => write!(f, ">>"),
            Token::Eof => write!(f, "EOF"),
        }
    }
}

pub struct Lexer<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn new_at(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos.min(self.data.len());
    }

    pub fn remaining(&self) -> &'a [u8] {
        if self.pos < self.data.len() {
            &self.data[self.pos..]
        } else {
            &[]
        }
    }

    /// Peeks at the next byte without consuming it.
    pub fn peek_byte(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Consumes and returns the next byte.
    pub fn next_byte(&mut self) -> Option<u8> {
        if self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    /// Skips whitespace and PDF comments (`%` up to newline).
    pub fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_pdf_whitespace(b) {
                self.pos += 1;
            } else if b == b'%' {
                // Comment: skip until CR or LF
                self.pos += 1;
                while self.pos < self.data.len() {
                    let c = self.data[self.pos];
                    if c == b'\r' || c == b'\n' {
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    /// Reads the next PDF token.
    pub fn next_token(&mut self) -> Result<Token> {
        self.skip_whitespace_and_comments();

        let b = match self.next_byte() {
            Some(byte) => byte,
            None => return Ok(Token::Eof),
        };

        match b {
            b'[' => Ok(Token::ArrayOpen),
            b']' => Ok(Token::ArrayClose),
            b'<' => {
                if self.peek_byte() == Some(b'<') {
                    self.pos += 1;
                    Ok(Token::DictOpen)
                } else {
                    self.read_hex_string()
                }
            }
            b'>' => {
                if self.peek_byte() == Some(b'>') {
                    self.pos += 1;
                    Ok(Token::DictClose)
                } else {
                    Err(PdfError::ParseError("Unexpected '>' delimiter".into()))
                }
            }
            b'(' => self.read_literal_string(),
            b'/' => self.read_name(),
            _ => self.read_number_or_keyword(b),
        }
    }

    /// Reads a literal string `(...)` taking into account escape sequences and balanced parens.
    fn read_literal_string(&mut self) -> Result<Token> {
        let mut depth = 1;
        let mut bytes = Vec::new();

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;

            if b == b'\\' {
                if self.pos >= self.data.len() {
                    break;
                }
                let esc = self.data[self.pos];
                self.pos += 1;
                match esc {
                    b'n' => bytes.push(b'\n'),
                    b'r' => bytes.push(b'\r'),
                    b't' => bytes.push(b'\t'),
                    b'b' => bytes.push(0x08),
                    b'f' => bytes.push(0x0C),
                    b'(' => bytes.push(b'('),
                    b')' => bytes.push(b')'),
                    b'\\' => bytes.push(b'\\'),
                    b'\r' => {
                        // Line continuation: \ followed by \r\n or \r ignores newline
                        if self.peek_byte() == Some(b'\n') {
                            self.pos += 1;
                        }
                    }
                    b'\n' => {
                        // Line continuation: \ followed by \n
                    }
                    b'0'..=b'7' => {
                        // Octal sequence (up to 3 octal digits)
                        let mut octal = (esc - b'0') as u32;
                        let mut count = 1;
                        while count < 3 && self.pos < self.data.len() {
                            let next = self.data[self.pos];
                            if (b'0'..=b'7').contains(&next) {
                                octal = (octal << 3) | ((next - b'0') as u32);
                                self.pos += 1;
                                count += 1;
                            } else {
                                break;
                            }
                        }
                        bytes.push((octal & 0xFF) as u8);
                    }
                    _ => {
                        // Non-standard escape: literal character
                        bytes.push(esc);
                    }
                }
            } else if b == b'(' {
                depth += 1;
                bytes.push(b);
            } else if b == b')' {
                depth -= 1;
                if depth == 0 {
                    return Ok(Token::StringLiteral(bytes));
                }
                bytes.push(b);
            } else {
                bytes.push(b);
            }
        }

        Err(PdfError::ParseError("Unterminated literal string".into()))
    }

    /// Reads a hex string `<...>`
    fn read_hex_string(&mut self) -> Result<Token> {
        let mut bytes = Vec::new();
        let mut first_nibble: Option<u8> = None;

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            self.pos += 1;

            if b == b'>' {
                if let Some(nibble) = first_nibble {
                    bytes.push(nibble << 4);
                }
                return Ok(Token::HexString(bytes));
            }

            if is_pdf_whitespace(b) {
                continue;
            }

            let val = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => {
                    return Err(PdfError::ParseError(format!(
                        "Invalid character in hex string: 0x{:02X}",
                        b
                    )))
                }
            };

            if let Some(high) = first_nibble.take() {
                bytes.push((high << 4) | val);
            } else {
                first_nibble = Some(val);
            }
        }

        Err(PdfError::ParseError("Unterminated hex string".into()))
    }

    /// Reads a PDF Name `/Name`
    fn read_name(&mut self) -> Result<Token> {
        let mut name = String::new();

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_pdf_whitespace(b) || is_pdf_delimiter(b) {
                break;
            }
            self.pos += 1;

            if b == b'#' {
                // Hex-encoded character `#XX`
                if self.pos + 2 <= self.data.len() {
                    let hex_slice = &self.data[self.pos..self.pos + 2];
                    if let Ok(hex_str) = std::str::from_utf8(hex_slice) {
                        if let Ok(byte_val) = u8::from_str_radix(hex_str, 16) {
                            name.push(byte_val as char);
                            self.pos += 2;
                            continue;
                        }
                    }
                }
                name.push('#');
            } else {
                name.push(b as char);
            }
        }

        Ok(Token::Name(name))
    }

    /// Reads a numeric token (Integer or Real) or a keyword.
    fn read_number_or_keyword(&mut self, first_byte: u8) -> Result<Token> {
        let mut buf = Vec::new();
        buf.push(first_byte);

        while self.pos < self.data.len() {
            let b = self.data[self.pos];
            if is_pdf_whitespace(b) || is_pdf_delimiter(b) {
                break;
            }
            self.pos += 1;
            buf.push(b);
        }

        let s = std::str::from_utf8(&buf)
            .map_err(|_| PdfError::ParseError("Invalid UTF-8 token".into()))?;

        // Attempt integer parse
        if let Ok(val) = s.parse::<i64>() {
            return Ok(Token::Integer(val));
        }

        // Attempt real parse
        if let Ok(val) = s.parse::<f64>() {
            return Ok(Token::Real(val));
        }

        // Keyword
        Ok(Token::Keyword(s.to_string()))
    }
}

#[inline]
pub fn is_pdf_whitespace(b: u8) -> bool {
    matches!(b, 0x00 | 0x09 | 0x0A | 0x0C | 0x0D | 0x20)
}

#[inline]
pub fn is_pdf_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_tokens() {
        let input = b"<< /Type /Pages /Count 3 /Kids [ 1 0 R 2 0 R ] >>";
        let mut lexer = Lexer::new(input);

        assert_eq!(lexer.next_token().unwrap(), Token::DictOpen);
        assert_eq!(lexer.next_token().unwrap(), Token::Name("Type".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::Name("Pages".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::Name("Count".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::Integer(3));
        assert_eq!(lexer.next_token().unwrap(), Token::Name("Kids".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::ArrayOpen);
        assert_eq!(lexer.next_token().unwrap(), Token::Integer(1));
        assert_eq!(lexer.next_token().unwrap(), Token::Integer(0));
        assert_eq!(lexer.next_token().unwrap(), Token::Keyword("R".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::Integer(2));
        assert_eq!(lexer.next_token().unwrap(), Token::Integer(0));
        assert_eq!(lexer.next_token().unwrap(), Token::Keyword("R".into()));
        assert_eq!(lexer.next_token().unwrap(), Token::ArrayClose);
        assert_eq!(lexer.next_token().unwrap(), Token::DictClose);
        assert_eq!(lexer.next_token().unwrap(), Token::Eof);
    }

    #[test]
    fn test_lexer_strings() {
        let input = b"(Hello (nested) World!) <48656c6c6f>";
        let mut lexer = Lexer::new(input);

        match lexer.next_token().unwrap() {
            Token::StringLiteral(bytes) => {
                assert_eq!(String::from_utf8(bytes).unwrap(), "Hello (nested) World!");
            }
            other => panic!("Expected StringLiteral, got {:?}", other),
        }

        match lexer.next_token().unwrap() {
            Token::HexString(bytes) => {
                assert_eq!(String::from_utf8(bytes).unwrap(), "Hello");
            }
            other => panic!("Expected HexString, got {:?}", other),
        }
    }
}
