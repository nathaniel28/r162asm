use std::fmt;
use std::io;
use std::num;

use crate::utf8;

#[derive(Debug)]
pub enum LexerError {
	LiteralTooLong,
	ParseIntError,
}

impl From<num::ParseIntError> for LexerError {
	fn from(_: num::ParseIntError) -> LexerError {
		LexerError::ParseIntError
	}
}

// TODO: drop special treatment of '(', ')', ',', and ':'
// let them just get put in Char
pub enum Token {
	Identifier(String),
	NumericLiteral(i64),
	OpenParenthesis,
	CloseParenthesis,
	Comma,
	Colon,
	EOF,
	Char(char),
}

impl fmt::Display for Token {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Identifier(id) => write!(f, "Identifier({})", id),
			Self::NumericLiteral(lit) => write!(f, "NumericLiteral({})", lit),
			Self::OpenParenthesis => write!(f, "("),
			Self::CloseParenthesis => write!(f, ")"),
			Self::Comma => write!(f, ","),
			Self::Colon => write!(f, ":"),
			Self::EOF => write!(f, "EOF"),
			Self::Char(c) => write!(f, "Char({})", c),
		}
	}
}

pub struct Lexer<T: io::Read> {
	rd: utf8::Reader<T>,
	unget: char,
}

impl<T: io::Read> Lexer<T> {
	const MAX_LITERAL_LEN: usize = 256;

	pub fn new(rd: T) -> Self {
		Self {
			rd: utf8::Reader::new(rd),
			unget: '\0',
		}
	}

	fn unget(&mut self, c: char) {
		if !c.is_whitespace() {
			self.unget = c;
		}
	}

	pub fn next(&mut self) -> Result<Token, LexerError> {
		let c;
		if self.unget == '\0' {
			loop {
				match self.rd.next() {
					Some(x) => if !x.is_whitespace() {
						c = x;
						break;
					}
					_ => return Ok(Token::EOF),
				}
			}
		} else {
			c = self.unget;
			self.unget = '\0';
		}
		match c {
			'#' => loop {
				// comment
				match self.rd.next() {
					Some(x) => if x == '\n' {
						return self.next();
					}
					_ => return Ok(Token::EOF),
				};
			}
			'_' | 'a'..='z' | 'A'..='Z' => {
				let mut buf = String::with_capacity(16);
				buf.push(c);
				loop {
					match self.rd.next() {
						Some(x) => if x.is_alphanumeric() || x == '_' {
							if buf.len() >= Self::MAX_LITERAL_LEN - 1 {
								return Err(LexerError::LiteralTooLong);
							}
							buf.push(x);
						} else {
							self.unget(x);
							break;
						}
						_ => break,
					}
				}
				Ok(Token::Identifier(buf))
			}
			'-' | '0'..='9' => {
				let mut radix = 10;
				let mut buf = String::with_capacity(16);
				buf.push(c);
				loop {
					match self.rd.next() {
						Some(x) => if x.is_ascii_hexdigit() {
							if buf.len() >= Self::MAX_LITERAL_LEN - 1 {
								return Err(LexerError::LiteralTooLong);
							}
							buf.push(x);
						} else if x == 'x' && buf.len() == 1 {
							radix = 16;
							buf.clear();
						} else if x != '_' {
							self.unget(x);
							break;
						}
						_ => break,
					}
				}
				Ok(Token::NumericLiteral(i64::from_str_radix(&buf, radix)?))
			}
			'(' => Ok(Token::OpenParenthesis),
			')' => Ok(Token::CloseParenthesis),
			',' => Ok(Token::Comma),
			':' => Ok(Token::Colon),
			_ => Ok(Token::Char(c)),
		}
	}
}
