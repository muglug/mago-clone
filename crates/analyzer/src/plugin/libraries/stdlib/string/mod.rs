//! PHP string function providers.

mod get_class;
mod sprintf;
mod strlen;

pub use get_class::GetClassProvider;
pub use sprintf::SprintfProvider;
pub use sprintf::resolve_sprintf;
pub use strlen::StrlenProvider;
