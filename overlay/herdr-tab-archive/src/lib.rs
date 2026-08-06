//! Searchable archived-tab picker for the Herdr fork.
//!
//! Herdr supplies current-workspace rows carrying raw tab indices. This crate
//! owns only the pure selection flow and presentation data.

pub mod flow;
pub mod view;

pub use flow::{Action, Key, PickerState, Row};
pub use view::{Segment, Tone};
