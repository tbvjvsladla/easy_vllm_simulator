//! A text file opened with `encoding="utf-8"` and iterated line by line, as `TextIOWrapper` does it: the bytes are decoded 8192 at a time with
//! an incremental UTF-8 decoder and universal-newline translation, so WHERE a decoding error surfaces (and which byte chunk it names) is the
//! chunk boundary's, and the lines before it are delivered first.
//!
//! The bytes come from a [`ByteSource`]: the whole content already in memory ([`MemorySource`], the original constructor and semantics) or a
//! reader that is read only when the decoder needs another chunk ([`ReadSource`]: one `read(8192)` per fill, as `BufferedReader.read1(8192)` is
//! asked by `TextIOWrapper`, so a pipe or FIFO gives short chunks and a line is delivered without waiting for the end of the stream).

use std::collections::VecDeque;
use std::convert::Infallible;
use std::io::Read;

const BYTE_CHUNK: usize = 8192;

/// A `UnicodeDecodeError` of the chunk being decoded: the bytes (an incomplete sequence of the chunk before included), the run and the reason.
#[derive(Debug, Clone)]
pub struct DecodeFailure {
    pub object: Vec<u8>,
    pub start: usize,
    pub end: usize,
    pub reason: &'static str,
}

impl DecodeFailure {
    /// `str(UnicodeDecodeError)`.
    pub fn message(&self) -> String {
        let subject = if self.end - self.start == 1 {
            format!(
                "byte 0x{:02x} in position {}",
                self.object[self.start], self.start
            )
        } else {
            format!("bytes in position {}-{}", self.start, self.end - 1)
        };
        format!("'utf-8' codec can't decode {subject}: {}", self.reason)
    }
}

/// Where the bytes of a [`TextStream`] come from: one chunk of at most 8192 bytes per call, an empty chunk at the end.
pub trait ByteSource {
    type Error;
    fn read_chunk(&mut self) -> Result<Vec<u8>, Self::Error>;
}

/// A content that is already in memory.
pub struct MemorySource {
    bytes: Vec<u8>,
    next_byte: usize,
}

impl ByteSource for MemorySource {
    type Error = Infallible;

    fn read_chunk(&mut self) -> Result<Vec<u8>, Infallible> {
        let end = (self.next_byte + BYTE_CHUNK).min(self.bytes.len());
        let chunk = self.bytes[self.next_byte..end].to_vec();
        self.next_byte = end;
        Ok(chunk)
    }
}

/// A reader read lazily: one `read(8192)` per chunk (an interrupted call is repeated, as CPython repeats it).
pub struct ReadSource<R: Read> {
    reader: R,
}

impl<R: Read> ByteSource for ReadSource<R> {
    type Error = std::io::Error;

    fn read_chunk(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let mut buffer = vec![0u8; BYTE_CHUNK];
        loop {
            match self.reader.read(&mut buffer) {
                Ok(count) => {
                    buffer.truncate(count);
                    return Ok(buffer);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

/// Why a line could not be delivered: the bytes do not decode, or the source failed to read.
#[derive(Debug)]
pub enum StreamFailure<E> {
    Decode(DecodeFailure),
    Read(E),
}

pub struct TextStream<S: ByteSource = MemorySource> {
    source: S,
    /// Bytes of an incomplete UTF-8 sequence waiting for the next chunk.
    pending_bytes: Vec<u8>,
    /// A `\r` at the end of a chunk, held until the next chunk shows whether a `\n` follows.
    pending_cr: bool,
    decoded: VecDeque<char>,
    finished: bool,
}

impl TextStream<MemorySource> {
    pub fn new(bytes: Vec<u8>) -> TextStream<MemorySource> {
        TextStream::from_source(MemorySource {
            bytes,
            next_byte: 0,
        })
    }

    /// `next(f)` over a content in memory: the next line with its `\n` (the last line may have none); `None` at the end. Decodes another chunk only
    /// when no complete line is buffered.
    pub fn next_line(&mut self) -> Result<Option<String>, DecodeFailure> {
        match self.next_line_checked() {
            Ok(line) => Ok(line),
            Err(StreamFailure::Decode(failure)) => Err(failure),
            Err(StreamFailure::Read(never)) => match never {},
        }
    }
}

impl<R: Read> TextStream<ReadSource<R>> {
    /// A stream over a reader that is read only as the lines are asked for.
    pub fn from_reader(reader: R) -> TextStream<ReadSource<R>> {
        TextStream::from_source(ReadSource { reader })
    }
}

impl<S: ByteSource> TextStream<S> {
    fn from_source(source: S) -> TextStream<S> {
        TextStream {
            source,
            pending_bytes: Vec::new(),
            pending_cr: false,
            decoded: VecDeque::new(),
            finished: false,
        }
    }

    /// Decodes one more chunk into `decoded`; `true` when the stream is at its end (the chunk was empty).
    fn fill(&mut self) -> Result<bool, StreamFailure<S::Error>> {
        if self.finished {
            return Ok(true);
        }
        let chunk = self.source.read_chunk().map_err(StreamFailure::Read)?;
        let is_final = chunk.is_empty();
        let mut input = std::mem::take(&mut self.pending_bytes);
        input.extend_from_slice(&chunk);
        let text = match std::str::from_utf8(&input) {
            Ok(text) => text.to_string(),
            Err(error) if error.error_len().is_none() && !is_final => {
                let valid = error.valid_up_to();
                self.pending_bytes = input[valid..].to_vec();
                std::str::from_utf8(&input[..valid])
                    .expect("validated prefix")
                    .to_string()
            }
            Err(error) => {
                let (start, length, reason) =
                    agent_runtime_support::pyjson::utf8_error_parts(&input, &error);
                return Err(StreamFailure::Decode(DecodeFailure {
                    object: input,
                    start,
                    end: start + length,
                    reason,
                }));
            }
        };
        // IncrementalNewlineDecoder(translate=True)
        let mut chars = text.chars().peekable();
        if self.pending_cr {
            self.pending_cr = false;
            self.decoded.push_back('\n');
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
        }
        while let Some(c) = chars.next() {
            if c == '\r' {
                match chars.peek() {
                    Some('\n') => {
                        chars.next();
                        self.decoded.push_back('\n');
                    }
                    Some(_) => self.decoded.push_back('\n'),
                    None if is_final => self.decoded.push_back('\n'),
                    None => self.pending_cr = true,
                }
            } else {
                self.decoded.push_back(c);
            }
        }
        if is_final {
            self.finished = true;
        }
        Ok(is_final)
    }

    /// `next(f)`: the next line with its `\n` (the last line may have none); `None` at the end. Reads and decodes another chunk only when no
    /// complete line is buffered, so a line that is already complete is delivered without touching the source again.
    pub fn next_line_checked(&mut self) -> Result<Option<String>, StreamFailure<S::Error>> {
        loop {
            if let Some(at) = self.decoded.iter().position(|&c| c == '\n') {
                return Ok(Some(self.decoded.drain(..=at).collect()));
            }
            if self.fill()? {
                if self.decoded.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(self.decoded.drain(..).collect()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// A source that hands out scripted chunks and then a scripted failure; counts the reads it was asked for.
    struct Scripted {
        steps: std::vec::IntoIter<Result<Vec<u8>, std::io::ErrorKind>>,
        reads: Rc<Cell<usize>>,
    }

    impl ByteSource for Scripted {
        type Error = std::io::ErrorKind;

        fn read_chunk(&mut self) -> Result<Vec<u8>, std::io::ErrorKind> {
            self.reads.set(self.reads.get() + 1);
            self.steps.next().unwrap_or(Ok(Vec::new()))
        }
    }

    fn scripted(
        steps: Vec<Result<Vec<u8>, std::io::ErrorKind>>,
    ) -> (TextStream<Scripted>, Rc<Cell<usize>>) {
        let reads = Rc::new(Cell::new(0));
        let source = Scripted {
            steps: steps.into_iter(),
            reads: reads.clone(),
        };
        (TextStream::from_source(source), reads)
    }

    #[test]
    fn a_complete_line_is_delivered_without_asking_the_source_again() {
        let (mut stream, reads) = scripted(vec![
            Ok(b"first\nsecond\nth".to_vec()),
            Err(std::io::ErrorKind::TimedOut),
        ]);
        assert_eq!(
            stream.next_line_checked().unwrap().as_deref(),
            Some("first\n")
        );
        assert_eq!(
            stream.next_line_checked().unwrap().as_deref(),
            Some("second\n")
        );
        assert_eq!(reads.get(), 1, "the buffered lines needed no second read");
        // the third line is incomplete: the source is asked, and its failure is the failure of that `next`
        assert!(matches!(
            stream.next_line_checked(),
            Err(StreamFailure::Read(std::io::ErrorKind::TimedOut))
        ));
        assert_eq!(reads.get(), 2);
    }

    #[test]
    fn short_reads_give_the_lines_of_one_whole_read_with_split_sequences_and_split_line_ends() {
        let text = "a\r\nb\rc\n\u{20ac}\u{1f600}x\r".as_bytes().to_vec();
        let mut whole = TextStream::new(text.clone());
        let mut expected = Vec::new();
        while let Some(line) = whole.next_line().unwrap() {
            expected.push(line);
        }
        for size in [1usize, 2, 3, 5] {
            let steps: Vec<_> = text.chunks(size).map(|chunk| Ok(chunk.to_vec())).collect();
            let (mut stream, _) = scripted(steps);
            let mut got = Vec::new();
            while let Some(line) = stream.next_line_checked().unwrap() {
                got.push(line);
            }
            assert_eq!(got, expected, "chunks of {size} bytes");
        }
        assert_eq!(expected, ["a\n", "b\n", "c\n", "\u{20ac}\u{1f600}x\n"]);
    }

    #[test]
    fn a_decoding_error_names_the_bytes_the_decoder_was_handed_and_a_read_error_comes_after_the_lines_before_it(
    ) {
        let (mut stream, _) = scripted(vec![Ok(b"ok\n".to_vec()), Ok(vec![b'x', 0xff, b'\n'])]);
        assert_eq!(stream.next_line_checked().unwrap().as_deref(), Some("ok\n"));
        let Err(StreamFailure::Decode(failure)) = stream.next_line_checked() else {
            panic!("a decoding error")
        };
        assert_eq!(
            (failure.start, failure.end, failure.object),
            (1, 2, vec![b'x', 0xff, b'\n'])
        );
        // an incomplete sequence is held for the next chunk; the end of the stream with it pending is `unexpected end of data`
        let (mut stream, _) = scripted(vec![Ok(vec![b'a', 0xe2, 0x82])]);
        let Err(StreamFailure::Decode(failure)) = stream.next_line_checked() else {
            panic!("a decoding error")
        };
        assert_eq!(
            (failure.reason, failure.start, failure.end),
            ("unexpected end of data", 0, 2)
        );
    }

    #[test]
    fn a_reader_is_read_in_chunks_of_8192_and_an_interrupted_read_is_repeated() {
        struct Flaky {
            sent: usize,
            interrupted: bool,
            sizes: Vec<usize>,
        }
        impl Read for Flaky {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.sizes.push(buffer.len());
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(std::io::ErrorKind::Interrupted.into());
                }
                if self.sent >= 3 {
                    return Ok(0);
                }
                buffer[..2].copy_from_slice(b"a\n");
                self.sent += 1;
                Ok(2)
            }
        }
        let mut stream = TextStream::from_reader(Flaky {
            sent: 0,
            interrupted: false,
            sizes: Vec::new(),
        });
        assert_eq!(stream.next_line_checked().unwrap().as_deref(), Some("a\n"));
        assert!(stream.source.reader.sizes.iter().all(|&size| size == 8192));
        assert_eq!(
            stream.source.reader.sizes.len(),
            2,
            "one interrupted call, one repeated"
        );
    }
}
