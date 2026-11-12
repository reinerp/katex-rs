//! Support for the hand-written builders of the DOM node, options and
//! settings types.
//!
//! Each builder keeps its optional fields in an inner struct and records
//! whether each required field has been set in its type, so calling `build`
//! before setting a required field does not compile.

/// Type-state marker for a required builder field that has not been set.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unset;

/// Generates a `name(value)` setter and a `maybe_name(Option<value>)` setter
/// for each optional field, stored as an `Option` in `self.opt`.
macro_rules! optional_setters {
    ($($(#[doc = $doc:literal])* $name:ident / $maybe:ident : $ty:ty;)*) => {$(
        $(#[doc = $doc])*
        #[allow(clippy::missing_const_for_fn, reason = "some field types have destructors")]
        pub fn $name(mut self, value: $ty) -> Self {
            self.opt.$name = Some(value);
            self
        }

        $(#[doc = $doc])*
        ///
        /// `None` leaves the field at its default.
        #[allow(clippy::missing_const_for_fn, reason = "some field types have destructors")]
        pub fn $maybe(mut self, value: Option<$ty>) -> Self {
            self.opt.$name = value;
            self
        }
    )*};
}

pub(crate) use optional_setters;
