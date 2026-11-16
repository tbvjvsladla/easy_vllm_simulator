//! `claim_predicates.py` command line: no arguments = run every clause predicate and print the JSON verdict
//! (exit 0 PASS / 1 FAIL). Additions (not in the original): `--clause ID` (repeatable) runs only those clauses;
//! `--list` prints the port status of every clause (JSON lines) and exits 0.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut only = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--list" => {
                println!("{}", policy_predicates::claims::list());
                std::process::exit(0);
            }
            "--clause" if i + 1 < args.len() => {
                only.push(args[i + 1].clone());
                i += 1;
            }
            other => {
                eprintln!("claim_predicates: unknown argument {other:?} (usage: claim_predicates [--clause ID]... | --list)");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let (payload, code) = policy_predicates::claims::run_all(&only);
    println!("{payload}");
    std::process::exit(code);
}
