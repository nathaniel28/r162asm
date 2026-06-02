use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;

mod utf8;
mod lexer;
mod parser;
use parser::*;

fn main() -> io::Result<()> {
	// TODO: actual argument parsing and error reporting
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
