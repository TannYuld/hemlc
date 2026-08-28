mod engine;
mod cssgen;
mod htmlgen;
mod jsgen;
mod types;
mod util;

pub use types::Compiler;
pub use types::minify::{self, None, Js, All};

pub const HEML_SCOPE_ATTRIBUTE_KEY: &str = "data-heml-scope";
pub const HEML_ID_ATTRIBUTE_KEY: &str = "data-heml-id";
