mod cssgen;
mod engine;
mod htmlgen;
mod jsgen;
mod types;
mod util;

use std::cell::LazyCell;

use regex::Regex;
pub use types::Compiler;
pub use types::minify::{self, All, Js, None};

const JS_IMPORT_REGEX: LazyCell<Regex> = LazyCell::new(|| {
    Regex::new(
        r#"(?x)
        (
            //.*
            | /\*(?s:.*?)\*/
            | "(?:[^"\\]|\\.)*"
            | '(?:[^'\\]|\\.)*'
            | `(?:[^`\\]|\\.)*`
        )
        |
        (
            \bimport\s+(?:[\w\s{},*$]+\s+from\s+)?(?:'[^']+'|"[^"]+")\s*;?
        )
        "#
    ).expect("Failed to compile parser regex")
});

pub const HEML_SCOPE_ATTRIBUTE_KEY: &str = "data-heml-scope";
pub const HEML_ID_ATTRIBUTE_KEY: &str = "data-heml-id";
