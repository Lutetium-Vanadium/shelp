use super::Repl;
use crate::LangInterface;

/// A wrapper over [`Repl`] which allows it to be used as a `Iterator`.
///
/// Although using an iterator is easier, errors are discarded and [`None`] is returned. For this
/// reason, it may be beneficial to use the [`Repl`] directly.
pub struct ReplIter<L: LangInterface> {
    repl: Repl<L>,
    color: crate::Color,
}

impl<L: LangInterface> ReplIter<L> {
    /// Create a iterator for a [Repl]
    pub fn new(repl: Repl<L>, color: crate::Color) -> Self {
        Self { repl, color }
    }
}

impl<L: LangInterface> Repl<L> {
    /// Shorthand to get iterator from self
    pub fn iter(self, color: crate::Color) -> ReplIter<L> {
        ReplIter::new(self, color)
    }
}

impl<L: LangInterface> Iterator for ReplIter<L> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.repl.next(self.color).ok()
    }
}
