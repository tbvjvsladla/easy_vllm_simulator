//! Python `SystemExit` as the A2A control-plane programs raise it: `raise SystemExit("msg")` prints `msg` + newline on
//! standard error and exits 1; `raise SystemExit(n)` / `sys.exit(n)` exits `n` silently. Shared by relay, agent_control,
//! bootstrap_canary and the gates so one module's in-process call into another keeps the original exit semantics.

#[derive(Debug, Clone, PartialEq)]
pub enum Exit {
    /// `SystemExit(int)`.
    Code(i32),
    /// `SystemExit(str)`: the message goes to standard error, status 1.
    Msg(String),
}

pub type Res<T> = Result<T, Exit>;

impl Exit {
    pub fn msg(m: impl Into<String>) -> Self {
        Exit::Msg(m.into())
    }
    /// What the interpreter does with an uncaught `SystemExit`: print a message (if any) and return the status.
    pub fn finish(&self) -> i32 {
        use std::io::Write;
        match self {
            Exit::Code(c) => *c,
            Exit::Msg(m) => {
                let _ = std::io::stdout().flush();
                eprintln!("{m}");
                1
            }
        }
    }
}

/// Convert a `main` result: `Ok(n)` returns `n`, `Err` behaves as an uncaught `SystemExit`.
pub fn run(r: Res<i32>) -> i32 {
    match r {
        Ok(c) => c,
        Err(e) => e.finish(),
    }
}
