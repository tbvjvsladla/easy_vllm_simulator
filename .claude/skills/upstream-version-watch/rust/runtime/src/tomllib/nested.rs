//! `NestedDict` of `tomllib/_parser.py`: the document (or inline table) being built, addressed by key paths.

use super::flags::Key;
use super::value::{Table, TomlValue};

/// Why a key path cannot be opened.
#[derive(Debug, PartialEq, Eq)]
pub enum NestError {
    /// The original's `KeyError`: a value that is not a table (or an array of tables) is in the way.
    NotANest,
    /// The original's `IndexError`: `cont[-1]` of an empty array. The parser only opens arrays that were created by `[[header]]` (never empty) or
    /// are frozen inline arrays, which its flag checks reject first, so no document reaches this; it is kept to follow the source.
    EmptyArray,
}

/// The parsed content of the document.
#[derive(Debug, Default)]
pub struct NestedDict {
    root: Table,
}

impl NestedDict {
    pub fn new() -> Self {
        NestedDict::default()
    }

    pub fn into_table(self) -> Table {
        self.root
    }

    /// `get_or_create_nest(key, access_lists=...)`: creates every missing table on the path and returns the last one.
    /// With `access_lists` an array on the path stands for its last element.
    pub fn get_or_create_nest(
        &mut self,
        key: &[Vec<u32>],
        access_lists: bool,
    ) -> Result<&mut Table, NestError> {
        let mut container = &mut self.root;
        for part in key {
            container = match container.slot_or_new_table(part) {
                TomlValue::Table(table) => table,
                TomlValue::Array(items) if access_lists => match items.last_mut() {
                    Some(TomlValue::Table(table)) => table,
                    Some(_) => return Err(NestError::NotANest),
                    None => return Err(NestError::EmptyArray),
                },
                _ => return Err(NestError::NotANest),
            };
        }
        Ok(container)
    }

    /// `append_nest_to_list(key)`: a new empty table at the end of the array under `key`, which is created when missing.
    pub fn append_nest_to_list(&mut self, key: &Key) -> Result<(), NestError> {
        let (last, parents) = key.split_last().expect("an array header has a key");
        let container = self.get_or_create_nest(parents, true)?;
        match container.get_mut(last) {
            Some(TomlValue::Array(items)) => {
                items.push(TomlValue::Table(Table::new()));
                Ok(())
            }
            Some(_) => Err(NestError::NotANest),
            None => {
                container.insert_new(
                    last.clone(),
                    TomlValue::Array(vec![TomlValue::Table(Table::new())]),
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Vec<u32> {
        value.chars().map(u32::from).collect()
    }

    #[test]
    fn an_empty_array_on_the_path_is_the_originals_index_error() {
        let mut nest = NestedDict::new();
        nest.root
            .insert_new(text("a"), TomlValue::Array(Vec::new()));
        assert_eq!(
            nest.get_or_create_nest(&[text("a")], true).err(),
            Some(NestError::EmptyArray)
        );
        assert_eq!(
            nest.get_or_create_nest(&[text("a")], false).err(),
            Some(NestError::NotANest)
        );
    }

    #[test]
    fn a_scalar_on_the_path_is_the_originals_key_error() {
        let mut nest = NestedDict::new();
        nest.root.insert_new(text("a"), TomlValue::Bool(true));
        assert_eq!(
            nest.get_or_create_nest(&[text("a"), text("b")], true).err(),
            Some(NestError::NotANest)
        );
    }
}
