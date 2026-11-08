//! Message construction for text that may hold lone surrogates: `PyStr` pieces and plain `&str` pieces joined in
//! order, the way `%`-formatting concatenates them.

use crate::pystr::PyStr;

/// A piece of a message.
pub(crate) trait Piece {
    fn push_onto(&self, out: &mut PyStr);
}

impl Piece for str {
    fn push_onto(&self, out: &mut PyStr) {
        out.push_str(self);
    }
}

impl Piece for String {
    fn push_onto(&self, out: &mut PyStr) {
        out.push_str(self);
    }
}

impl Piece for PyStr {
    fn push_onto(&self, out: &mut PyStr) {
        out.push_py(self);
    }
}

impl<T: Piece + ?Sized> Piece for &T {
    fn push_onto(&self, out: &mut PyStr) {
        (**self).push_onto(out);
    }
}

/// `cat!("a", pystr, format!(..))` -> one `PyStr`.
macro_rules! cat {
    ($($piece:expr),* $(,)?) => {{
        let mut out = $crate::pystr::PyStr::default();
        $( $crate::repeat_text::Piece::push_onto(&$piece, &mut out); )*
        out
    }};
}
pub(crate) use cat;
