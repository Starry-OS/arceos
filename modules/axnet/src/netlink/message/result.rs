use axerrno::{AxError, ax_err_type};

/// Result of reading a protocol-specific segment.
#[derive(Debug)]
pub enum ContinueRead<T, E = AxError> {
    Parsed(T),
    Skipped,
    SkippedErr(E),
}

impl<T> ContinueRead<T, AxError> {
    /// Creates a [`SkippedErr`] variant with the given error information.
    ///
    /// [`SkippedErr`]: Self::SkippedErr
    pub fn skipped_with_error(errno: AxError, msg: &'static str) -> Self {
        Self::SkippedErr(ax_err_type!(errno, msg))
    }
}

impl<T, E> ContinueRead<T, E> {
    /// Maps a `ContinueRead<T, E>` to `ContinueRead<U, E>` by applying a
    /// function to a contained `Parsed` value.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ContinueRead<U, E> {
        match self {
            ContinueRead::Parsed(t) => ContinueRead::Parsed(f(t)),
            ContinueRead::Skipped => ContinueRead::Skipped,
            ContinueRead::SkippedErr(e) => ContinueRead::SkippedErr(e),
        }
    }

    /// Maps a `ContinueRead<T, E>` to `ContinueRead<T, U>` by applying a
    /// function to a contained `SkippedErr` value.
    pub fn map_err<F, U>(self, f: F) -> ContinueRead<T, U>
    where
        F: FnOnce(E) -> U,
    {
        match self {
            Self::Parsed(val) => ContinueRead::Parsed(val),
            Self::Skipped => ContinueRead::Skipped,
            Self::SkippedErr(err) => ContinueRead::SkippedErr(f(err)),
        }
    }
}
