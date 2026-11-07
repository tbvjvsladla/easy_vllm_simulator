//! Standard output as CPython 3.12 buffers it, and the failures a write can report.
//!
//! A `print` reaches the descriptor only through two buffers: the text layer collects encoded bytes until 8192 are pending
//! (a write that would pass the limit first sends what is pending), and the byte layer (as large as the descriptor's block
//! size, 4096 for a pipe or `/dev/full`) keeps anything that fits and writes whatever does not. So a short result is only
//! written when the interpreter exits (a failure there is an "Exception ignored" report and exit status 120), while a result
//! of 8192 bytes or more fails inside the `print` that produced it (an uncaught `OSError`, status 1). [`TextStdout`] reproduces
//! exactly that sequence over any [`Write`] sink, and every write, read and flush result is looked at, never dropped.

use agent_runtime_support::failure::Failure;
use std::io::Write;

/// The text layer's chunk size (`TextIOWrapper._CHUNK_SIZE`).
const TEXT_CHUNK: usize = 8192;
/// The byte layer's size when the descriptor reports no usable block size (`io.DEFAULT_BUFFER_SIZE`).
pub const DEFAULT_BUFFER: usize = 8192;

/// The text layer plus the byte layer of one output stream.
pub struct TextStdout<W: Write> {
    raw: W,
    pending: Vec<u8>,
    buffer: Vec<u8>,
    capacity: usize,
    line_buffering: bool,
}

impl<W: Write> TextStdout<W> {
    pub fn new(raw: W, capacity: usize) -> Self {
        TextStdout {
            raw,
            pending: Vec::new(),
            buffer: Vec::new(),
            capacity: capacity.max(2),
            line_buffering: false,
        }
    }

    /// A terminal's stream is line-buffered: a write containing `\n` or `\r` flushes both layers and the descriptor
    /// (`TextIOWrapper.write`'s `needflush`).
    pub fn line_buffered(mut self, on: bool) -> Self {
        self.line_buffering = on;
        self
    }

    /// `sys.stdout.write(text)` for text already encoded.
    pub fn write(&mut self, data: &[u8]) -> std::io::Result<()> {
        if !self.pending.is_empty() && self.pending.len() + data.len() > TEXT_CHUNK {
            self.write_pending()?;
        }
        self.pending.extend_from_slice(data);
        let needs_flush =
            self.line_buffering && data.iter().any(|&byte| byte == b'\n' || byte == b'\r');
        if self.pending.len() >= TEXT_CHUNK || needs_flush {
            self.write_pending()?;
        }
        if needs_flush {
            self.flush_buffer()?;
        }
        Ok(())
    }

    /// The byte layer's `flush`: what it holds goes to the descriptor.
    fn flush_buffer(&mut self) -> std::io::Result<()> {
        let held = std::mem::take(&mut self.buffer);
        if !held.is_empty() {
            self.raw.write_all(&held)?;
        }
        self.raw.flush()
    }

    /// Hands the pending text to the byte layer (the pending bytes are gone whether or not the write succeeds).
    fn write_pending(&mut self) -> std::io::Result<()> {
        let bytes = std::mem::take(&mut self.pending);
        if bytes.len() <= self.capacity - self.buffer.len() {
            self.buffer.extend_from_slice(&bytes);
            return Ok(());
        }
        let held = std::mem::take(&mut self.buffer);
        if !held.is_empty() {
            self.raw.write_all(&held)?;
        }
        if bytes.len() > self.capacity {
            self.raw.write_all(&bytes)
        } else {
            self.buffer = bytes;
            Ok(())
        }
    }

    /// The flush at interpreter exit: the text layer, then the byte layer, then the descriptor.
    pub fn finish(&mut self) -> std::io::Result<()> {
        self.write_pending()?;
        self.flush_buffer()
    }
}

/// The exception CPython raises for an operating-system error on an open stream (no path in the message).
pub fn os_failure(error: &std::io::Error) -> Failure {
    let class = match error.raw_os_error() {
        Some(1 | 13) => "PermissionError",
        Some(32) => "BrokenPipeError",
        Some(9) => "OSError",
        _ => "OSError",
    };
    let detail = error.to_string();
    let strerror = detail
        .split(" (os error")
        .next()
        .unwrap_or(&detail)
        .to_string();
    let message = match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {strerror}"),
        None => strerror,
    };
    Failure::new(class, &message)
}

/// The inherited standard output as a raw sink (a duplicate of descriptor 1, written without Rust's own line buffering)
/// and the byte-layer size the interpreter would pick for it.
pub fn inherited_stdout() -> std::io::Result<(std::fs::File, usize)> {
    use std::os::fd::AsFd;
    use std::os::unix::fs::MetadataExt;
    let file = std::fs::File::from(std::io::stdout().as_fd().try_clone_to_owned()?);
    let capacity = file
        .metadata()
        .ok()
        .map(|metadata| metadata.blksize() as usize)
        .filter(|&size| size > 1)
        .unwrap_or(DEFAULT_BUFFER);
    Ok((file, capacity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    /// A sink that fails every write, as `/dev/full` does, remembering how often it was asked.
    struct Full(usize);

    impl Write for Full {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            self.0 += 1;
            Err(Error::from_raw_os_error(28))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_short_result_is_only_written_at_exit_and_a_long_one_inside_the_print() {
        let mut short = TextStdout::new(Full(0), 4096);
        assert!(short.write(&[b'a'; 600]).is_ok() && short.write(b"\n").is_ok());
        assert_eq!(short.raw.0, 0, "nothing reached the sink yet");
        assert_eq!(short.finish().unwrap_err().raw_os_error(), Some(28));
        let mut long = TextStdout::new(Full(0), 4096);
        assert_eq!(
            long.write(&[b'a'; 8192]).unwrap_err().raw_os_error(),
            Some(28)
        );
        let mut middle = TextStdout::new(Full(0), 4096);
        assert!(
            middle.write(&[b'a'; 5000]).is_ok(),
            "held in the text layer below 8192"
        );
        assert!(
            middle.finish().is_err(),
            "5000 bytes pass the 4096-byte byte layer at exit"
        );
    }

    #[test]
    fn a_line_buffered_stream_reaches_the_sink_at_each_newline_not_at_exit() {
        let mut terminal = TextStdout::new(Full(0), 4096).line_buffered(true);
        assert_eq!(
            terminal.write(b"short").map(|()| terminal.raw.0).unwrap(),
            0,
            "no newline, nothing written"
        );
        assert_eq!(
            terminal.write(b"\n").unwrap_err().raw_os_error(),
            Some(28),
            "the newline flushes inside the write, so the failure is the write's"
        );
        let mut carriage = TextStdout::new(Full(0), 4096).line_buffered(true);
        assert!(
            carriage.write(b"progress\r").is_err(),
            "a carriage return flushes too"
        );
        let mut pipe = TextStdout::new(Full(0), 4096);
        assert!(
            pipe.write(b"short\n").is_ok(),
            "a block-buffered stream keeps it until exit"
        );
    }

    #[test]
    fn a_working_sink_gets_every_byte_in_order() {
        let mut sink = Vec::new();
        {
            let mut out = TextStdout::new(&mut sink, 4096);
            for index in 0..3000u32 {
                out.write(format!("line {index}").as_bytes()).unwrap();
                out.write(b"\n").unwrap();
            }
            out.finish().unwrap();
        }
        let text = String::from_utf8(sink).unwrap();
        assert_eq!(text.lines().count(), 3000);
        assert!(text.starts_with("line 0\nline 1\n") && text.ends_with("line 2999\n"));
    }

    #[test]
    fn the_failure_reads_as_python_words_it() {
        let failure = os_failure(&Error::from_raw_os_error(28));
        assert_eq!(
            (failure.class, failure.message.to_utf8().unwrap()),
            ("OSError", "[Errno 28] No space left on device".to_string())
        );
        assert_eq!(
            os_failure(&Error::from_raw_os_error(32)).class,
            "BrokenPipeError"
        );
    }
}
