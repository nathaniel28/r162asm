use std::io;

// makes a lot of small calls to read on rd,
// so it's best to use a BufReader as T or similar
pub struct Reader<T: io::Read> {
	rd: io::Bytes<T>,
}

impl<T: io::Read> Reader<T> {
	pub fn new(rd: T) -> Self {
		Self {
			rd: rd.bytes(),
		}
	}

	// returns the next utf-8 char in the stream
	// invalid characters are discarded silently
	// returns None at EOF on on error
	pub fn next(&mut self) -> Option<char> {
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
