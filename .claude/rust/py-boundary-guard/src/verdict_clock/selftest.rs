//! `--self-test`: the readers against the planted fixtures of `checks/data/verdict_clock/` (compiled in), which must
//! give exactly the reads of `expected.tsv` (fixture, line, text). Every fixture has one expected read at least,
//! every row of `expected.tsv` names a fixture, and the reads checked are as many as its rows.

use super::{find, Finding, Surface};

type Fixture<'a> = (&'a str, Surface, &'a str);

const FIXTURES: [Fixture<'static>; 3] = [
    ("planted.rs", Surface::Rust, include_str!("../../checks/data/verdict_clock/planted.rs")),
    ("planted.sh", Surface::Shell, include_str!("../../checks/data/verdict_clock/planted.sh")),
    ("planted.ts", Surface::TypeScript, include_str!("../../checks/data/verdict_clock/planted.ts")),
];
const EXPECTED: &str = include_str!("../../checks/data/verdict_clock/expected.tsv");

fn expected_of(expected: &str, fixture: &str) -> Result<Vec<Finding>, String> {
    expected
        .lines()
        .filter(|row| row.split('\t').next() == Some(fixture))
        .map(|row| match row.split('\t').collect::<Vec<_>>()[..] {
            [_, line, text] => line
                .parse()
                .map(|line| Finding { line, text: text.to_string() })
                .map_err(|_| format!("expected.tsv: bad line number in {row:?}")),
            _ => Err(format!("expected.tsv: expected <fixture>\\t<line>\\t<text>: {row:?}")),
        })
        .collect()
}

pub fn run() -> Result<usize, Vec<String>> {
    run_on(&FIXTURES, EXPECTED)
}

fn run_on(fixtures: &[Fixture], expected: &str) -> Result<usize, Vec<String>> {
    let mut failures = Vec::new();
    let rows: Vec<&str> = expected.lines().filter(|row| !row.is_empty()).collect();
    for row in &rows {
        let fixture = row.split('\t').next().unwrap_or_default();
        if !fixtures.iter().any(|(name, _, _)| *name == fixture) {
            failures.push(format!("expected.tsv: unknown fixture {fixture:?}: {row:?}"));
        }
    }
    let mut checked = 0;
    for (name, surface, source) in fixtures {
        let mut want = match expected_of(expected, name) {
            Ok(want) if want.is_empty() => {
                failures.push(format!("{name}: no expected read in expected.tsv"));
                continue;
            }
            Ok(want) => want,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        match find(*surface, source) {
            Ok(mut got) => {
                got.sort_by(|a, b| (a.line, &a.text).cmp(&(b.line, &b.text)));
                want.sort_by(|a, b| (a.line, &a.text).cmp(&(b.line, &b.text)));
                if got == want {
                    checked += got.len();
                } else {
                    failures.push(format!("{name}: want {want:?}\n  got {got:?}"));
                }
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    if failures.is_empty() && checked != rows.len() {
        failures.push(format!("checked {checked} reads, expected.tsv has {} rows", rows.len()));
    }
    if failures.is_empty() {
        Ok(checked)
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    use super::{run, run_on, Surface};

    const PLANTED_READS: usize = 52;

    #[test]
    fn planted_fixtures_give_the_expected_reads() {
        assert_eq!(run().map_err(|f| f.join("\n")), Ok(PLANTED_READS));
    }

    #[test]
    fn an_empty_or_unknown_fixture_fails() {
        let one = [("a.ts", Surface::TypeScript, "Date.now();\n")];
        assert_eq!(run_on(&one, "a.ts\t1\tDate.now()\n"), Ok(1));
        assert!(run_on(&one, "").is_err());
        assert!(run_on(&one, "a.ts\t1\tDate.now()\nb.ts\t1\tDate.now()\n").is_err());
        let two = [("a.ts", Surface::TypeScript, "Date.now();\n"), ("b.ts", Surface::TypeScript, "f();\n")];
        assert!(run_on(&two, "a.ts\t1\tDate.now()\n").is_err());
    }
}
