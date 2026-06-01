use std::io;
use std::num;
use std::fmt;
use std::collections::{HashMap, hash_map};

// makes a lot of small calls to read on rd,
// so it's best to use a BufReader as T or similar
struct CharReader<T: io::Read> {
	rd: io::Bytes<T>,
}

impl<T: io::Read> CharReader<T> {
	fn new(rd: T) -> Self {
		Self {
			rd: rd.bytes(),
		}
	}

	// returns the next utf-8 char in the stream
	// invalid characters are discarded silently
	// returns None at EOF on on error
	fn next(&mut self) -> Option<char> {
		let mut b0;
		loop {
			b0 = self.rd.next()?.ok()?;
			if b0 & 0xc0 != 0x80 {
				break;
			}
			// it's a continuation byte (thus invalid), so get a new one
		}
		let mut expect_continue = || {
			let b = self.rd.next()?.ok()?;
			return if b & 0xc0 == 0x80 {
				Some((b & 0x3f) as u32)
			} else {
				None
			};
		};
		match b0 {
			0x00..0x80 => char::from_u32(b0 as u32),
			0x80..0xc0 => unreachable!(),
			0xc0..0xe0 => {
				// 2 byte char
				let b0 = (b0 & 0x1f) as u32;
				let b1 = expect_continue()?;
				char::from_u32(b0 << 6 | b1)
			}
			0xe0..0xf0 => {
				// 3 byte char
				let b0 = (b0 & 0x1f) as u32;
				let b1 = expect_continue()?;
				let b2 = expect_continue()?;
				char::from_u32(b0 << 12 | b1 << 6 | b2)
			}
			0xf0..0xf8 => {
				// 4 bytes char
				let b0 = (b0 & 0x1f) as u32;
				let b1 = expect_continue()?;
				let b2 = expect_continue()?;
				let b3 = expect_continue()?;
				char::from_u32(b0 << 18 | b1 << 12 | b2 << 6 | b3)
			}
			_ => None,
		}
	}
}

#[derive(Debug)]
enum LexerError {
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
enum Token {
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

struct Lexer<T: io::Read> {
	rd: CharReader<T>,
	unget: char,
}

impl<T: io::Read> Lexer<T> {
	const MAX_LITERAL_LEN: usize = 256;

	fn new(rd: T) -> Self {
		Self {
			rd: CharReader::new(rd),
			unget: '\0',
		}
	}

	fn unget(&mut self, c: char) {
		if !c.is_whitespace() {
			self.unget = c;
		}
	}

	fn next(&mut self) -> Result<Token, LexerError> {
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

struct LabelWaiter {
	fix_offset: u64,
	partial: u16,
}

enum Label {
	Waiters(Vec<LabelWaiter>),
	Value(u64),
}

// TODO
struct ParserError();

// TODO
impl From<io::Error> for ParserError {
	fn from(_err: io::Error) -> ParserError {
		ParserError()
	}
}

struct Parser<R: io::Read, W: io::Write + io::Seek> {
	lexer: Lexer<R>,
	emitter: W,
	labels: HashMap<String, Label>,
	pos: u64,
}

impl<R: io::Read, W: io::Write + io::Seek> Parser<R, W> {
	fn new(rd: R, wr: W) -> Self {
		Self {
			lexer: Lexer::new(rd),
			emitter: wr,
			labels: HashMap::with_capacity(32),
			pos: 0,
		}
	}

	// return the value of a label if it has been defined
	// otherwise, add a label waiter and return 0
	fn resolve_label(&mut self, label: String, partial: u16) -> u64 {
		match self.labels.entry(label).or_insert(Label::Waiters(Vec::new())) {
			Label::Value(x) => *x,
			Label::Waiters(arr) => {
				arr.push(LabelWaiter {
					fix_offset: self.pos,
					partial,
				});
				0
			}
		}
	}

	fn emit(&mut self, instr: u16) -> io::Result<()> {
		self.emitter.write_all(&[instr as u8, (instr >> 8) as u8])?;
		self.pos += 1;
		Ok(())
	}

	fn error(&self, args: &fmt::Arguments<'_>) {
		// TODO: mention line and column, make lexer track these
		println!("{}", args);
	}

	fn lex(&mut self) -> Result<Token, ParserError> {
		self.lexer.next().map_err(|e| {
			self.error(&format_args!("the lexer encountered an error: {:?}", e));
			ParserError()
		})
	}

	fn expect_comma(&mut self) -> Result<(), ParserError> {
		match self.lex()? {
			Token::Comma => Ok(()),
			e => {
				self.error(&format_args!("unexpected token {}: expected a Comma instead", e));
				Err(ParserError())
			}
		}
	}

	// range is inclusive
	fn expect_n_on_range(&mut self, want_lo: i64, want_hi: i64) -> Result<i64, ParserError> {
		let lit = match self.lex()? {
			Token::NumericLiteral(x) => x,
			e => {
				self.error(&format_args!("unexpected token {}: expected a NumericLiteral instead", e));
				return Err(ParserError());
			}
		};
		if lit < want_lo || lit > want_hi {
			self.error(&format_args!("expected a numeric literal on the range {}..={}, found {} instead", want_lo, want_hi, lit));
			return Err(ParserError());
		}
		Ok(lit)
	}

	fn expect_register_id(&mut self) -> Result<u16, ParserError> {
		let id = match self.lex()? {
			Token::Identifier(x) => x,
			e => {
				self.error(&format_args!("unexpected token {}: expected an Identifier instead", e));
				return Err(ParserError());
			}
		};
		Ok(match &id[..] {
			"x0" | "zero" => 0,
			"x1" | "ra" => 1,
			"x2" | "t0" => 2,
			"x3" | "t1" => 3,
			"x4" | "t2" => 4,
			"x5" | "t3" => 5,
			"x6" | "a0" => 6,
			"x7" | "a1" => 7,
			"x8" | "a2" => 8,
			"x9" | "a3" => 9,
			"x10" | "s0" => 10,
			"x11" | "s1" => 11,
			"x12" | "s2" => 12,
			"x13" | "s3" => 13,
			"x14" | "s4" => 14,
			"x15" | "sp" => 15,
			_ => {
				self.error(&format_args!("expected a register identifier, found {} instead", id));
				return Err(ParserError());
			}
		})
	}

	fn expect_label_or_imm(&mut self, instr: u16) -> Result<i64, ParserError> {
		Ok(match self.lex()? {
			Token::Identifier(id) => {
				let there = self.resolve_label(id, instr);
				there.wrapping_sub(self.pos) as i64
			}
			Token::NumericLiteral(x) => x,
			e => {
				self.error(&format_args!("unexpected token {}: expected an Identifier or a NumericLiteral instead", e));
				return Err(ParserError());
			}
		})
	}

	fn fmt_imm_si(imm: u16) -> u16 {
		debug_assert!(imm < 16);
		imm << 8
	}

	fn fmt_imm_ls(imm: u16) -> u16 {
		debug_assert!(imm < 8);
		imm << 5
	}

	fn fmt_imm_b(imm: u16) -> u16 {
		debug_assert!(imm < 256);
		imm << 4
	}

	fn fmt_imm_i(imm: u16) -> u16 {
		debug_assert!(imm < 512);
		(imm & 0x0ff) << 4 | (imm & 0x100) >> 5
	}

	fn fmt_imm_j(imm: u16) -> u16 {
		debug_assert!(imm < 4096);
		(imm & 0xcff) << 4 | (imm & 0x300) >> 6
	}

	fn expect_sr(&mut self, op: u16) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		self.expect_comma()?;
		let b = self.expect_register_id()?;
		Ok(self.emit(
			0x00
			| op
			| b << 8
			| reg << 12
		)?)
	}

	fn expect_si(&mut self, op: u16) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		self.expect_comma()?;
		let imm = self.expect_n_on_range(0, 15)?;
		Ok(self.emit(
			0x00
			| op
			| Self::fmt_imm_si(imm as u16)
			| reg << 12
		)?)
	}

	fn expect_ls(&mut self, op: u16) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		self.expect_comma()?;
		let imm = self.expect_n_on_range(0, 7)?;
		match self.lex()? {
			Token::OpenParenthesis => {},
			e => {
				self.error(&format_args!("expected an OpenParenthesis, found {} instead", e));
				return Err(ParserError());
			}
		}
		let areg = self.expect_register_id()?;
		match self.lex()? {
			Token::CloseParenthesis => {},
			e => {
				self.error(&format_args!("expected a CloseParenthesis, found {} instead", e));
				return Err(ParserError());
			}
		}
		Ok(self.emit(
			0x10
			| op
			| Self::fmt_imm_ls(imm as u16)
			| areg << 8
			| reg << 12
		)?)
	}

	fn expect_b(&mut self, op: u16) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		self.expect_comma()?;
		let instr = 0x8 | op | reg << 12;
		let imm = self.expect_label_or_imm(instr)?;
		if imm < -128 || imm > 127 {
			self.error(&format_args!("branches can only jump on the range -128..=127, but offset {} was specified", imm));
			return Err(ParserError());
		}
		Ok(self.emit(instr | Self::fmt_imm_b(imm as u16 & 0xff))?)
	}

	fn expect_r(&mut self, op: u16) -> Result<(), ParserError> {
		let dst = self.expect_register_id()?;
		self.expect_comma()?;
		let a = self.expect_register_id()?;
		self.expect_comma()?;
		let b = self.expect_register_id()?;
		Ok(self.emit(
			0x2
			| op
			| a << 4
			| b << 8
			| dst << 12
		)?)
	}

	fn expect_i(&mut self, op: u16) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		self.expect_comma()?;
		let imm = self.expect_n_on_range(-256, 255)?;
		Ok(self.emit(
			0x1
			| op
			| Self::fmt_imm_i(imm as u16 & 0x1ff)
			| reg << 12
		)?)
	}

	fn expect_j(&mut self) -> Result<(), ParserError> {
		let reg = self.expect_register_id()?;
		if reg >= 4 {
			self.error(&format_args!("jal can only link to registers on the range x0..=x3, but register x{} was specified", reg));
			return Err(ParserError());
		}
		self.expect_comma()?;
		let instr = 0x3 | reg << 12;
		let imm = self.expect_label_or_imm(instr)?;
		if imm < -2048 || imm > 2047 {
			self.error(&format_args!("jal can only jump on the range -2048..=2047, but offset {} was specified", imm));
			return Err(ParserError());
		}
		let instr = instr | Self::fmt_imm_j(imm as u16 & 0xfff);
		Ok(self.emit(instr)?)
	}

	fn expect_label(&mut self, name: String) -> Result<(), ParserError> {
		match self.lex()? {
			Token::Colon => {}
			e => {
				self.error(&format_args!("unexpected token {}: expected Colon instead", e));
				return Err(ParserError());
			}
		}
		match self.labels.entry(name) {
			hash_map::Entry::Occupied(mut x) => match x.get() {
				Label::Value(_) => {
					// clone only required to satisfy borrow checker
					let l = x.key().clone();
					self.error(&format_args!("redefinition of label {}", l));
					return Err(ParserError());
				}
				Label::Waiters(arr) => {
					for i in 0..arr.len() {
						self.emitter.seek(
							io::SeekFrom::Start(2 * arr[i].fix_offset)
						)?;
						let fixed = if arr[i].partial & 0x3 == 0x3 {
							// it's jal
							Self::fmt_imm_j(
								(self.pos
									.wrapping_sub(arr[i].fix_offset)
									.wrapping_sub(1) & 0xfff
								) as u16
							)
						} else {
							debug_assert!(arr[i].partial & 0xb == 0x8);
							// it's a branch
							Self::fmt_imm_b(
								(self.pos
									.wrapping_sub(arr[i].fix_offset)
									.wrapping_sub(1) & 0xff
								) as u16
							)
						} | arr[i].partial;
						self.emitter.write_all(
							&[fixed as u8, (fixed >> 8) as u8]
						)?;
					}
					self.emitter.seek(io::SeekFrom::Start(2 * self.pos))?;
					x.insert(Label::Value(self.pos.wrapping_sub(1)));
				}
			}
			hash_map::Entry::Vacant(x) => {
				x.insert(Label::Value(self.pos.wrapping_sub(1)));
			}
		}
		Ok(())
	}

	fn parse(&mut self) -> Result<(), ParserError> {
		loop {
			let id = match self.lex()? {
				Token::Identifier(x) => x,
				Token::EOF => break,
				e => {
					self.error(&format_args!("unexpected token {}: expected an Identifier instead", e));
					return Err(ParserError());
				}
			};
			match &id[..] {
				"sll" => self.expect_sr(0x00)?,
				"sra" => self.expect_sr(0x04)?,
				"srl" => self.expect_sr(0x40)?,
				"xor" => self.expect_sr(0x44)?,
				"or" => self.expect_sr(0x80)?,
				"and" => self.expect_sr(0x84)?,
				"mul" => self.expect_sr(0xc0)?,
				"mulh" => self.expect_sr(0xc4)?,
				"div" => self.expect_sr(0xe0)?,
				"jalr" => self.expect_sr(0xe4)?,
				"slli" => self.expect_si(0x20)?,
				"srai" => self.expect_si(0x24)?,
				"srli" => self.expect_si(0x60)?,
				"xori" => self.expect_si(0x64)?,
				"ori" => self.expect_si(0xa0)?,
				"andi" => self.expect_si(0xa4)?,
				"sw" => self.expect_ls(0x0)?,
				"lw" => self.expect_ls(0x4)?,
				"bnez" => self.expect_b(0x0)?,
				"beqz" => self.expect_b(0x4)?,
				"add" => self.expect_r(0x0)?,
				"sub" => self.expect_r(0x4)?,
				"slt" => self.expect_r(0x8)?,
				"sltu" => self.expect_r(0xc)?,
				"addi" => self.expect_i(0x0)?,
				"li" => self.expect_i(0x4)?,
				"jal" => self.expect_j()?,
				_ => self.expect_label(id)?,
			}
		}
		// TODO: loop over labels to check if any were unresolved
		// if so, that's an error
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn test(id: i32, given: &str, expect: &[u16]) {
		let mut buf = io::Cursor::new(Vec::<u8>::with_capacity(2 * expect.len()));
		let mut parser = Parser::new(given.as_bytes(), &mut buf);
		if parser.parse().is_err() {
			panic!("{}: parser encountered an error", id);
		}
		let buf = buf.into_inner();
		if buf.len() != 2 * expect.len() {
			panic!("{}: length of output ({}) and expected output ({}) do not match", id, buf.len(), 2 * expect.len());
		}
		for i in 0..expect.len() {
			let v = buf[2 * i] as u16 | (buf[2 * i + 1] as u16) << 8;
			if v != expect[i] {
				panic!("{}: offset {}: expected 0x{:04x}, found 0x{:04x}", id, i, expect[i], v);
			}
		}
	}

	#[test]
	fn parse_label() {
		test(0, "before:\njal x0, before", &[0xcfff]);
		test(1, "a:\nbnez x0, a", &[0x0ff8]);
		// TODO: another branches test
		test(2, "before:
jal x0, before
jal x0, before
jal x0, before
jal x0, before
jal x0, before
jal x0, before
jal x0, before
jal x0, after
jal x0, after
jal x0, after
jal x0, after
jal x0, after
jal x0, after
after:", &[0xcfff, 0xcfef, 0xcfdf, 0xcfcf, 0xcfbf, 0xcfaf, 0xcf9f, 0x0053, 0x0043, 0x0033, 0x0023, 0x0013, 0x0003]);
	}

	// TODO: simple multi-instruction tests
	/*
	#[test]
	fn parse_multiple() {
	}
	*/

	#[test]
	fn parse_register_names() {
		test(0, "sll zero, ra", &[0x0100]);
		test(1, "sll t0, t1", &[0x2300]);
		test(2, "sll t2, t3", &[0x4500]);
		test(3, "sll a0, a1", &[0x6700]);
		test(4, "sll a2, a3", &[0x8900]);
		test(5, "sll s0, s1", &[0xab00]);
		test(6, "sll s2, s3", &[0xcd00]);
		test(7, "sll s4, sp", &[0xef00]);
		test(8, "sll x0, x1", &[0x0100]);
		test(9, "sll x2, x3", &[0x2300]);
		test(10, "sll x4, x5", &[0x4500]);
		test(11, "sll x6, x7", &[0x6700]);
		test(12, "sll x8, x9", &[0x8900]);
		test(13, "sll x10, x11", &[0xab00]);
		test(14, "sll x12, x13", &[0xcd00]);
		test(15, "sll x14, x15", &[0xef00]);
	}

	#[test]
	fn parse_sr_type() {
		test(0, "sll x0, x0", &[0x0000]);
		test(1, "sll x1, x0", &[0x1000]);
		test(2, "sll x8, x0", &[0x8000]);
		test(3, "sll x15, x0", &[0xf000]);
		test(4, "sll x0, x1", &[0x0100]);
		test(5, "sll x0, x8", &[0x0800]);
		test(6, "sll x0, x15", &[0x0f00]);
		test(0, "sra x0, x0", &[0x0004]);
		test(0, "srl x0, x0", &[0x0040]);
		test(0, "xor x0, x0", &[0x0044]);
		test(0, "or x0, x0", &[0x0080]);
		test(0, "and x0, x0", &[0x0084]);
		test(0, "mul x0, x0", &[0x00c0]);
		test(0, "mulh x0, x0", &[0x00c4]);
		test(0, "div x0, x0", &[0x00e0]);
		test(0, "jalr x0, x0", &[0x00e4]);
	}

	#[test]
	fn parse_si_type() {
		test(0, "slli x0, 0", &[0x0020]);
		test(1, "slli x1, 0", &[0x1020]);
		test(2, "slli x8, 0", &[0x8020]);
		test(3, "slli x15, 0", &[0xf020]);
		test(4, "slli x0, 1", &[0x0120]);
		test(5, "slli x0, 5", &[0x0520]);
		test(6, "slli x0, 10", &[0x0a20]);
		test(7, "slli x0, 15", &[0x0f20]);
		test(8, "srai x0, 0", &[0x0024]);
		test(9, "srli x0, 0", &[0x0060]);
		test(10, "xori x0, 0", &[0x0064]);
		test(11, "ori x0, 0", &[0x00a0]);
		test(12, "andi x0, 0", &[0x00a4]);
	}

	#[test]
	fn parse_ls_type() {
		test(0, "sw x0, 0(x0)", &[0x0010]);
		test(1, "sw x1, 0(x0)", &[0x1010]);
		test(2, "sw x8, 0(x0)", &[0x8010]);
		test(3, "sw x15, 0(x0)", &[0xf010]);
		test(4, "sw x0, 0(x1)", &[0x0110]);
		test(5, "sw x0, 0(x8)", &[0x0810]);
		test(6, "sw x0, 0(x15)", &[0x0f10]);
		test(7, "sw x0, 1(x0)", &[0x0030]);
		test(8, "sw x0, 4(x0)", &[0x0090]);
		test(9, "sw x0, 7(x0)", &[0x00f0]);
		test(10, "lw x0, 0(x0)", &[0x0014]);
	}

	#[test]
	fn parse_b_type() {
		test(0, "bnez x0, 0", &[0x0008]);
		test(1, "bnez x1, 0", &[0x1008]);
		test(2, "bnez x8, 0", &[0x8008]);
		test(3, "bnez x15, 0", &[0xf008]);
		test(4, "bnez x0, 1", &[0x0018]);
		test(5, "bnez x0, 127", &[0x07f8]);
		test(6, "bnez x0, -128", &[0x0808]);
		test(7, "bnez x0, -1", &[0x0ff8]);
		test(8, "bnez x0, -103", &[0x0998]);
		test(9, "beqz x0, 0", &[0x000c]);
	}

	#[test]
	fn parse_r_type() {
		test(0, "add x0, x0, x0", &[0x0002]);
		test(1, "add x1, x0, x0", &[0x1002]);
		test(2, "add x15, x0, x0", &[0xf002]);
		test(3, "add x0, x1, x0", &[0x0012]);
		test(4, "add x0, x15, x0", &[0x00f2]);
		test(5, "add x0, x0, x1", &[0x0102]);
		test(6, "add x0, x0, x15", &[0x0f02]);
		test(7, "sub x0, x0, x0", &[0x0006]);
		test(8, "slt x0, x0, x0", &[0x000a]);
		test(9, "sltu x0, x0, x0", &[0x000e]);
	}

	#[test]
	fn parse_i_type() {
		test(0, "addi x0, 0", &[0x0001]);
		test(1, "addi x1, 0", &[0x1001]);
		test(2, "addi x8, 0", &[0x8001]);
		test(3, "addi x15, 0", &[0xf001]);
		test(4, "addi x0, 255", &[0x0ff1]);
		test(5, "addi x0, -1", &[0x0ff9]);
		test(6, "addi x0, -215", &[0x0299]);
		test(7, "addi x0, -256", &[0x0009]);
		test(8, "li x0, 0", &[0x0005]);
	}

	#[test]
	fn parse_j_type() {
		test(0, "jal x0, 0", &[0x0003]);
		test(1, "jal x1, 0", &[0x1003]);
		test(2, "jal x2, 0", &[0x2003]);
		test(3, "jal x3, 0", &[0x3003]);
		test(4, "jal x0, -549", &[0xcdb7]);
		test(5, "jal x0, -1639", &[0x8997]);
		test(6, "jal x0, -1", &[0xcfff]);
		test(7, "jal x0, -1912", &[0x8883]);
		test(8, "jal x0, 273", &[0x0117]);
		test(9, "jal x0, 2047", &[0x4fff]);
	}
}

// TODO: have you heard of using multiple files

fn main() -> io::Result<()> {
	// TODO: actual argument parsing and error reporting
	use std::env;
	use std::fs;
	use std::ffi::OsString;
	let argv: Vec<OsString> = env::args_os().collect();
	if argv.len() != 3 {
		println!("usage: {} <program> <output>", argv[0].to_string_lossy());
		return Ok(());
	}
	let mut parser = Parser::new(
		io::BufReader::new(fs::File::open(&argv[1])?),
		io::BufWriter::new(fs::File::create(&argv[2])?)
	);
	let _ = parser.parse();
	Ok(())
}
