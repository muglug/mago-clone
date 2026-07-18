//! PHP array function providers.

mod array_all;
mod array_column;
mod array_filter;
mod array_flip;
mod array_key_exists;
mod array_map;
mod array_merge;
mod array_pointer;
mod array_reverse;
mod array_shift_pop;
mod array_splice;
mod compact;
mod range;

pub use array_all::ArrayAllAssertionProvider;
pub use array_column::ArrayColumnProvider;
pub use array_filter::ArrayFilterProvider;
pub use array_flip::ArrayFlipProvider;
pub use array_key_exists::ArrayKeyExistsProvider;
pub use array_map::ArrayMapProvider;
pub use array_merge::ArrayMergeProvider;
pub use array_pointer::ArrayKeyProvider;
pub use array_pointer::ArrayPointerProvider;
pub use array_reverse::ArrayReverseProvider;
pub use array_shift_pop::ArrayShiftPopProvider;
pub use array_splice::ArraySpliceProvider;
pub use compact::CompactProvider;
pub use range::RangeProvider;
