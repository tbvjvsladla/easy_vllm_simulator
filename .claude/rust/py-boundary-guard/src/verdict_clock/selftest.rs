//! `--self-test`: the readers against the planted fixtures of `checks/data/verdict_clock/` (compiled in), which must
//! give exactly the reads of `expected.tsv` (fixture, line, text).

use super::{find, Finding, Surface};

const FIXTURES: [(&str, Surface, &str); 3] = [
    ("planted.rs", Surface::Rust, include_str!("../../checks/data/verdict_clock/planted.rs")),
    ("planted.sh", Surface::Shell, include_str!("../../checks/data/verdict_clock/planted.sh")),
    ("planted.ts", Surface::TypeScript, include_str!("../../checks/data/verdict_clock/planted.ts")),
];
const EXPECTED: &str = include_str!("../../checks/data/verdict_clock/expected.tsv");

fn expected_of(fixture: &str) -> Result<Vec<Finding>, String> {
    EXPECTED
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

/// The number of reads checked, or the mismatches.
pub fn run() -> Result<usize, Vec<String>> {
    let mut failures = Vec::new();
    let mut checked = 0;
    for (name, surface, source) in FIXTURES {
        let want = match expected_of(name) {
            Ok(want) => want,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        match find(surface, source) {
            Ok(mut got) => {
                got.sort_by(|a, b| (a.line, &a.text).cmp(&(b.line, &b.text)));
                let mut want = want;
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
    if failures.is_empty() {
        Ok(checked)
    } else {
        Err(failures)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn planted_fixtures_give_the_expected_reads() {
        assert_eq!(super::run().map_err(|f| f.join("\n")), Ok(21));
    }
}
