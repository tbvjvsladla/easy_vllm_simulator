//! py_exec_trap — plan_26111615 D3 "py 호출 0 가드" (라이브 증거 수집용).
//!
//! E2E 동안 `python3`·`python` 이름의 심볼릭 링크로 PATH 맨 앞에 둔다. 하네스가 호스트 Python 을
//! 띄우면 그 사실(argv · 부모 프로세스 cmdline · cwd · UTC)을 `PY_EXEC_TRAP_LOG` 에 JSON 한 줄로
//! 남기고 exit 97 로 실패시킨다(py 폴백 ✗ — 조용히 통과시키지 않는다). 컨테이너 안 Python(vLLM·GuideLLM)은
//! 컨테이너 PATH 를 쓰므로 이 트랩에 닿지 않는다.
//!
//! - `PY_EXEC_TRAP_LOG` 미설정 = 기록 불가 → 그래도 exit 97 (fail-closed).
//! - 기록 형식은 키 순서 고정의 한 줄 JSON(문자열은 최소 이스케이프).

use std::io::Write;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn cmdline(pid: &str) -> String {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|b| {
            b.split(|x| *x == 0)
                .filter(|p| !p.is_empty())
                .map(|p| String::from_utf8_lossy(p).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn ppid_of(pid: &str) -> Option<String> {
    let st = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // field 4 after the ")" that closes comm
    let rest = &st[st.rfind(')')? + 2..];
    rest.split_whitespace().nth(1).map(str::to_string)
}

fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (d, s) = ((secs / 86400) as i64, secs % 86400);
    // civil-from-days (Howard Hinnant)
    let z = d + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let mm = if mp < 10 { mp + 3 } else { mp - 9 };
    let yy = if mm <= 2 { y + 1 } else { y };
    format!(
        "{yy:04}-{mm:02}-{dd:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let parent = std::os::unix::process::parent_id().to_string();
    let grand = ppid_of(&parent).unwrap_or_default();
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let line = format!(
        "{{\"utc\":{},\"argv\":[{}],\"cwd\":{},\"parent_pid\":{},\"parent_cmdline\":{},\"grandparent_cmdline\":{}}}\n",
        esc(&utc_now()),
        argv.iter().map(|a| esc(a)).collect::<Vec<_>>().join(","),
        esc(&cwd),
        esc(&parent),
        esc(&cmdline(&parent)),
        esc(&if grand.is_empty() { String::new() } else { cmdline(&grand) }),
    );
    match std::env::var_os("PY_EXEC_TRAP_LOG") {
        Some(p) => {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
            {
                let _ = f.write_all(line.as_bytes());
            }
        }
        None => eprintln!("[py_exec_trap] PY_EXEC_TRAP_LOG 미설정 — 기록 없이 거부한다"),
    }
    eprintln!(
        "[py_exec_trap] 하네스가 호스트 Python 을 띄웠다(거부): {}",
        argv.join(" ")
    );
    std::process::exit(97);
}
