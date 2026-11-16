//! `hint_api`: JSON-lines library bridge (see `hintlib::api`). Reads requests from stdin, one response line each.
use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req = match hintlib::value::loads(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hint_api: bad request line: {e}");
                std::process::exit(2);
            }
        };
        let (r, o, e) = hintlib::api::dispatch(&req);
        let mut resp = hintlib::api::response(&req["id"], &r, &o, &e);
        if req.get("hex") == Some(&hintlib::value::J::Bool(true)) {
            // `"hex": true`: the response after the id is the hex of its UTF-8 JSON (keeps PII-shaped fixture results
            // out of tracked goldens as text; same rule as the capture shim).
            let mut body = resp.as_dict().cloned().unwrap_or_default();
            body.remove("id");
            let text = hintlib::value::dumps_unicode(&hintlib::value::J::Dict(body));
            let hex: String = text.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
            resp = hintlib::jd! {"id" => req["id"].clone(), "hex" => hex};
        }
        let mut so = std::io::stdout().lock();
        let _ = writeln!(so, "{}", hintlib::value::dumps_unicode(&resp));
        let _ = so.flush();
    }
}
