mod codegen;
mod htmlgen;
mod jsgen;
mod cssgen;
mod util;
mod types;

pub use types::{Compiler, CompilerOptions, CodegenStrategy};

pub const HEML_SCOPE_ATTRIBUTE_KEY: &'static str = "data-heml-scope";
pub const HEML_ID_ATTRIBUTE_KEY: &'static str = "data-heml-id";