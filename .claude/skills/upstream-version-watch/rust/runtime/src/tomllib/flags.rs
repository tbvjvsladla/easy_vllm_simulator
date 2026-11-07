//! `Flags` of `tomllib/_parser.py`: what each parsed key or namespace may no longer be used for.

use super::value::Text;
use std::collections::{HashMap, HashSet};

/// A parsed key path.
pub type Key = Vec<Text>;

/// The two flags of the original (`Flags.FROZEN`, `Flags.EXPLICIT_NEST`), as bit masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flag {
    /// An immutable namespace (inline array or inline table).
    Frozen = 1,
    /// A nest that was created explicitly and can no longer be opened by a `[table]` header.
    ExplicitNest = 2,
}

#[derive(Debug, Default)]
struct FlagNode {
    flags: u8,
    recursive_flags: u8,
    nested: HashMap<Text, FlagNode>,
}

/// The flags of every key path seen so far, plus the flags still pending until the next table header.
#[derive(Debug, Default)]
pub struct Flags {
    roots: HashMap<Text, FlagNode>,
    pending: HashSet<(Key, Flag)>,
}

impl Flags {
    pub fn new() -> Self {
        Flags::default()
    }

    pub fn add_pending(&mut self, key: Key, flag: Flag) {
        self.pending.insert((key, flag));
    }

    /// `finalize_pending`: the order the pending set is drained in does not matter, every entry is an idempotent flag insertion.
    pub fn finalize_pending(&mut self) {
        let pending: Vec<(Key, Flag)> = self.pending.drain().collect();
        for (key, flag) in pending {
            self.set(&key, flag, false);
        }
    }

    /// `unset_all`: forgets the flags of `key` and of everything below it; a missing ancestor leaves everything as it is.
    pub fn unset_all(&mut self, key: &[Text]) {
        let Some((stem, parents)) = key.split_last() else {
            return;
        };
        let mut container = &mut self.roots;
        for part in parents {
            match container.get_mut(part) {
                Some(node) => container = &mut node.nested,
                None => return,
            }
        }
        container.remove(stem);
    }

    /// `set(key, flag, recursive=...)`: creates the nodes along the path.
    pub fn set(&mut self, key: &[Text], flag: Flag, recursive: bool) {
        let Some((stem, parents)) = key.split_last() else {
            return;
        };
        let mut container = &mut self.roots;
        for part in parents {
            container = &mut container.entry(part.clone()).or_default().nested;
        }
        let node = container.entry(stem.clone()).or_default();
        if recursive {
            node.recursive_flags |= flag as u8;
        } else {
            node.flags |= flag as u8;
        }
    }

    /// `is_(key, flag)`: an ancestor's recursive flag counts, the document root has no flags.
    pub fn is(&self, key: &[Text], flag: Flag) -> bool {
        let Some((stem, parents)) = key.split_last() else {
            return false;
        };
        let mut container = &self.roots;
        for part in parents {
            let Some(node) = container.get(part) else {
                return false;
            };
            if node.recursive_flags & flag as u8 != 0 {
                return true;
            }
            container = &node.nested;
        }
        match container.get(stem) {
            Some(node) => (node.flags | node.recursive_flags) & flag as u8 != 0,
            None => false,
        }
    }
}
