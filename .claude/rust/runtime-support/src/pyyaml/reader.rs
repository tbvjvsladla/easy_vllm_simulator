//! `yaml/reader.py` for a text-file source, plus the text file underneath it.
//!
//! `yaml.safe_load(f)` reads `f` 4096 characters at a time through `TextIOWrapper.read`, which in turn decodes the
//! bytes 8192 at a time with an incremental UTF-8 decoder and universal-newline translation. Where those chunks
//! end decides when a decoding error is raised (and what byte position it names, relative to the chunk being
//! decoded), so the same chunking is reproduced.

use super::frames::{capture, frame, nested, PySite};
use super::{plain, Failure, Mark, Observer, Parsed, ReadEvent};
use crate::pystr::PyStr;
use std::rc::Rc;

const BYTE_CHUNK: usize = 8192;
const CHAR_CHUNK: usize = 4096;

/// A text file opened with `encoding="utf-8"`.
struct TextStream<'a> {
    bytes: &'a [u8],
    next_byte: usize,
    /// Bytes of an incomplete UTF-8 sequence waiting for the next chunk.
    pending_bytes: Vec<u8>,
    /// A `\r` at the end of a chunk, held until the next chunk shows whether a `\n` follows.
    pending_cr: bool,
    decoded: std::collections::VecDeque<char>,
    finished: bool,
}

impl<'a> TextStream<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        TextStream {
            bytes,
            next_byte: 0,
            pending_bytes: Vec::new(),
            pending_cr: false,
            decoded: std::collections::VecDeque::new(),
            finished: false,
        }
    }

    /// `TextIOWrapper.read_chunk`: decodes one more chunk into `decoded`. Returns whether the stream is at its end
    /// (the chunk was empty): asking again after the end asks the file and the decoder again.
    fn fill(&mut self) -> Parsed<bool> {
        // the Python-level decoder is asked for its state, then to decode
        nested(&["getstate"])?;
        if self.finished {
            nested(&["decode"])?;
            return Ok(true);
        }
        let end = (self.next_byte + BYTE_CHUNK).min(self.bytes.len());
        let chunk = &self.bytes[self.next_byte..end];
        self.next_byte = end;
        let is_final = chunk.is_empty();
        nested(&["decode"])?;
        let mut input = std::mem::take(&mut self.pending_bytes);
        input.extend_from_slice(chunk);
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
                let (start, length, reason) = crate::pyjson::utf8_error_parts(&input, &error);
                // the exception comes out of `codecs.IncrementalDecoder.decode` (line 322 of the frozen module), called by the reader's `update_raw`
                capture(Some(PySite {
                    file: "<frozen codecs>",
                    function: "decode",
                    line: 322,
                }));
                return Err(Failure::Decode {
                    object: input,
                    start,
                    end: start + length,
                    reason,
                });
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

    /// `read(n)`: up to `n` characters; fewer only at the end.
    fn read(&mut self, count: usize) -> Parsed<Vec<char>> {
        while self.decoded.len() < count {
            if self.fill()? {
                break;
            }
        }
        let take = count.min(self.decoded.len());
        Ok(self.decoded.drain(..take).collect())
    }
}

/// Identity tokens of the `Mark` objects (provenance only: no behaviour depends on their values, only on their equality).
static MARK_ORIGINS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) struct Reader<'a> {
    pub name: Rc<PyStr>,
    stream: TextStream<'a>,
    buffer: Vec<char>,
    pointer: usize,
    raw_buffer: Option<Vec<char>>,
    eof: bool,
    observer: Option<Observer>,
    pub index: usize,
    pub line: usize,
    pub column: usize,
}

fn printable(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{7E}' | '\u{85}' | '\u{A0}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

impl<'a> Reader<'a> {
    pub fn new(name: Rc<PyStr>, bytes: &'a [u8], observer: Option<Observer>) -> Parsed<Self> {
        let mut reader = Reader {
            name,
            stream: TextStream::new(bytes),
            buffer: Vec::new(),
            pointer: 0,
            raw_buffer: None,
            eof: false,
            observer,
            index: 0,
            line: 0,
            column: 0,
        };
        while !reader.eof && reader.raw_buffer.as_ref().is_none_or(|raw| raw.len() < 2) {
            reader.update_raw()?;
        }
        reader.update(1)?;
        Ok(reader)
    }

    /// `Reader.__init__(stream)` for a `str` source (what `yaml.safe_load(path.read_text())` gives it): the name is
    /// the caller's (`<unicode string>`), the whole text is checked for non-printable characters at once and becomes
    /// the buffer, with no raw buffer left to refill.
    pub fn from_text(name: Rc<PyStr>, text: &PyStr) -> Parsed<Self> {
        let points = text.code_points();
        let offset = points
            .iter()
            .position(|&p| char::from_u32(p).is_none_or(|c| !printable(c)));
        nested(if offset.is_some() {
            &["check_printable", "__init__"][..]
        } else {
            &["check_printable"][..]
        })?;
        if let Some(offset) = offset {
            return Err(Failure::Reader {
                name,
                position: offset,
                character: points[offset],
                reason: "special characters are not allowed",
            });
        }
        let mut buffer: Vec<char> = points.iter().filter_map(|&p| char::from_u32(p)).collect();
        buffer.push('\0');
        Ok(Reader {
            name,
            stream: TextStream::new(&[]),
            buffer,
            pointer: 0,
            raw_buffer: None,
            eof: true,
            observer: None,
            index: 0,
            line: 0,
            column: 0,
        })
    }

    fn tell(&self, event: ReadEvent) {
        if let Some(observer) = &self.observer {
            observer.borrow_mut().push(event);
        }
    }

    fn update_raw(&mut self) -> Parsed<()> {
        let _frame = frame("update_raw")?;
        self.tell(ReadEvent::Read { size: CHAR_CHUNK });
        let data = match self.stream.read(CHAR_CHUNK) {
            Ok(data) => data,
            Err(failure) => {
                let error = failure.clone().into_error();
                self.tell(ReadEvent::Error {
                    class: error.class,
                    message: error.message,
                });
                return Err(failure);
            }
        };
        self.tell(ReadEvent::Return {
            text: PyStr::new(&data.iter().collect::<String>()),
        });
        let empty = data.is_empty();
        match &mut self.raw_buffer {
            None => self.raw_buffer = Some(data),
            Some(raw) => raw.extend(data),
        }
        if empty {
            self.eof = true;
        }
        Ok(())
    }

    fn update(&mut self, length: usize) -> Parsed<()> {
        let _frame = frame("update")?;
        if self.raw_buffer.is_none() {
            return Ok(());
        }
        self.buffer.drain(..self.pointer);
        self.pointer = 0;
        while self.buffer.len() < length {
            if !self.eof {
                self.update_raw()?;
            }
            let data = std::mem::take(
                self.raw_buffer
                    .as_mut()
                    .expect("raw buffer is present until the end"),
            );
            // `check_printable(data)`; a bad character raises `ReaderError` inside it
            let offset = data.iter().position(|&c| !printable(c));
            nested(if offset.is_some() {
                &["check_printable", "__init__"][..]
            } else {
                &["check_printable"][..]
            })?;
            if let Some(offset) = offset {
                return Err(Failure::Reader {
                    name: self.name.clone(),
                    position: self.index + self.buffer.len() + offset,
                    character: u32::from(data[offset]),
                    reason: "special characters are not allowed",
                });
            }
            self.buffer.extend(data);
            if self.eof {
                self.buffer.push('\0');
                self.raw_buffer = None;
                break;
            }
        }
        Ok(())
    }

    pub fn peek(&mut self, offset: usize) -> Parsed<char> {
        let _frame = frame("peek")?;
        if self.pointer + offset >= self.buffer.len() {
            self.update(offset + 1)?;
        }
        self.buffer
            .get(self.pointer + offset)
            .copied()
            .ok_or_else(|| plain("IndexError", "string index out of range"))
    }

    pub fn prefix(&mut self, length: usize) -> Parsed<Vec<char>> {
        let _frame = frame("prefix")?;
        if self.pointer + length >= self.buffer.len() {
            self.update(length)?;
        }
        let end = (self.pointer + length).min(self.buffer.len());
        Ok(self.buffer[self.pointer..end].to_vec())
    }

    pub fn forward(&mut self, mut length: usize) -> Parsed<()> {
        let _frame = frame("forward")?;
        if self.pointer + length + 1 >= self.buffer.len() {
            self.update(length + 1)?;
        }
        while length > 0 {
            let c = *self
                .buffer
                .get(self.pointer)
                .ok_or_else(|| plain("IndexError", "string index out of range"))?;
            self.pointer += 1;
            self.index += 1;
            if matches!(c, '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
                || (c == '\r' && self.buffer.get(self.pointer) != Some(&'\n'))
            {
                self.line += 1;
                self.column = 0;
            } else if c != '\u{FEFF}' {
                self.column += 1;
            }
            length -= 1;
        }
        Ok(())
    }

    /// `get_mark()`: the call, and the `Mark.__init__` it makes.
    pub fn mark(&self) -> Parsed<Mark> {
        nested(&["get_mark", "__init__"])?;
        // `get_mark()` builds a NEW `yaml.error.Mark` object every time: a fresh origin
        Ok(Mark {
            name: self.name.clone(),
            index: self.index,
            line: self.line,
            column: self.column,
            origin: MARK_ORIGINS.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }
}
