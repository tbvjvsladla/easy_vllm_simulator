//! The byte channel under the HTTP parser, and the buffered reader that gives it the `socket.makefile("rb")` semantics `http.client` relies on.
//!
//! [`Wire`] is the seam to the real transport (see `transport.rs`): `pull` appends whatever the peer sent next and reports end of stream.
//! [`BufferedReader`] never asks the wire for more than a call needs: `readline(limit)` stops at the first line end, `read(n)` returns fewer
//! than `n` bytes only at end of stream, and a body that ends by its declared length is never read to EOF.

use super::failure::{value_error, Fetched, TransportFailure};

/// One open connection.
pub trait Wire: Send {
    /// Sends all of `bytes`.
    fn send_all(&mut self, bytes: &[u8]) -> Result<(), TransportFailure>;

    /// Appends the next bytes that arrive to `sink`: `Ok(true)` when at least one byte was appended, `Ok(false)` at end of stream.
    fn pull(&mut self, sink: &mut Vec<u8>) -> Result<bool, TransportFailure>;
}

/// Opens connections.
pub trait Dial {
    fn dial(&self, host: &str, port: u16) -> Result<Box<dyn Wire>, TransportFailure>;
}

/// The reader of one response: a buffer over a [`Wire`].
pub struct BufferedReader {
    wire: Box<dyn Wire>,
    buffer: Vec<u8>,
    consumed: usize,
    at_end: bool,
}

impl BufferedReader {
    pub fn new(wire: Box<dyn Wire>) -> BufferedReader {
        BufferedReader {
            wire,
            buffer: Vec::new(),
            consumed: 0,
            at_end: false,
        }
    }

    pub fn send_all(&mut self, bytes: &[u8]) -> Fetched<()> {
        Ok(self.wire.send_all(bytes)?)
    }

    /// Gives the connection back, dropping what was buffered but not read (as CPython drops the buffer of a `makefile` it closes).
    pub fn into_wire(self) -> Box<dyn Wire> {
        self.wire
    }

    fn available(&self) -> usize {
        self.buffer.len() - self.consumed
    }

    /// Takes more bytes from the wire; `false` once the peer has closed.
    fn refill(&mut self) -> Fetched<bool> {
        if self.at_end {
            return Ok(false);
        }
        if self.consumed == self.buffer.len() {
            self.buffer.clear();
            self.consumed = 0;
        }
        if !self.wire.pull(&mut self.buffer)? {
            self.at_end = true;
            return Ok(false);
        }
        Ok(true)
    }

    fn take(&mut self, count: usize) -> Vec<u8> {
        let taken = self.buffer[self.consumed..self.consumed + count].to_vec();
        self.consumed += count;
        taken
    }

    /// `fp.readline(limit)`: up to `limit` bytes, ending after a `\n`; shorter only at end of stream.
    pub fn read_line(&mut self, limit: usize) -> Fetched<Vec<u8>> {
        let mut searched = 0;
        loop {
            let pending = &self.buffer[self.consumed..];
            if let Some(offset) = pending[searched..].iter().position(|&byte| byte == b'\n') {
                let length = (searched + offset + 1).min(limit);
                return Ok(self.take(length));
            }
            if pending.len() >= limit {
                return Ok(self.take(limit));
            }
            searched = pending.len();
            if !self.refill()? {
                let length = self.available();
                return Ok(self.take(length));
            }
        }
    }

    /// `fp.read(amount)` for `amount` of `-1` (everything) or a non-negative size; any other negative size is the `ValueError` of
    /// `BufferedReader`.
    pub fn read(&mut self, amount: i64) -> Fetched<Vec<u8>> {
        if amount < -1 {
            return Err(value_error("read length must be non-negative or -1").into());
        }
        if amount == -1 {
            while self.refill()? {}
            let length = self.available();
            return Ok(self.take(length));
        }
        let wanted = usize::try_from(amount).expect("non-negative size");
        while self.available() < wanted {
            if !self.refill()? {
                break;
            }
        }
        let length = self.available().min(wanted);
        Ok(self.take(length))
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// A wire that delivers scripted pieces, one per `pull`, then end of stream (the test plane's isolation of the parser from sockets).
    pub struct Scripted {
        pub pieces: std::collections::VecDeque<Vec<u8>>,
        pub pulls: usize,
        pub sent: Vec<u8>,
    }

    impl Scripted {
        pub fn reader(pieces: &[&[u8]]) -> BufferedReader {
            BufferedReader::new(Box::new(Scripted {
                pieces: pieces.iter().map(|piece| piece.to_vec()).collect(),
                pulls: 0,
                sent: Vec::new(),
            }))
        }
    }

    impl Wire for Scripted {
        fn send_all(&mut self, bytes: &[u8]) -> Result<(), TransportFailure> {
            self.sent.extend_from_slice(bytes);
            Ok(())
        }

        fn pull(&mut self, sink: &mut Vec<u8>) -> Result<bool, TransportFailure> {
            self.pulls += 1;
            match self.pieces.pop_front() {
                Some(piece) => {
                    sink.extend_from_slice(&piece);
                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::Scripted;

    #[test]
    fn read_line_stops_at_the_newline_and_the_limit() {
        let mut reader = Scripted::reader(&[b"ab", b"c\nde", b"f\n"]);
        assert_eq!(reader.read_line(100).unwrap(), b"abc\n");
        assert_eq!(reader.read_line(2).unwrap(), b"de");
        assert_eq!(reader.read_line(100).unwrap(), b"f\n");
        assert_eq!(reader.read_line(100).unwrap(), b"");
    }

    #[test]
    fn read_returns_short_only_at_the_end_of_the_stream() {
        let mut reader = Scripted::reader(&[b"he", b"llo"]);
        assert_eq!(reader.read(4).unwrap(), b"hell");
        assert_eq!(reader.read(10).unwrap(), b"o");
        assert_eq!(reader.read(10).unwrap(), b"");
    }

    #[test]
    fn read_to_the_declared_end_does_not_wait_for_more() {
        let mut reader = Scripted::reader(&[b"hello", b"never pulled"]);
        assert_eq!(reader.read(5).unwrap(), b"hello");
    }

    #[test]
    fn negative_sizes_follow_buffered_reader() {
        let mut reader = Scripted::reader(&[b"abc", b"def"]);
        assert_eq!(reader.read(-1).unwrap(), b"abcdef");
        assert!(reader.read(-2).is_err());
    }
}
