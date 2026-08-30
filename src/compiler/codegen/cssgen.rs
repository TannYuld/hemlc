use crate::compiler::codegen::{HEML_SCOPE_ATTRIBUTE_KEY, types::{CSSGenerator, minify}};

impl CSSGenerator for minify::None {
    
}

impl CSSGenerator for minify::Js {
    
}

impl CSSGenerator for minify::All {
    
}