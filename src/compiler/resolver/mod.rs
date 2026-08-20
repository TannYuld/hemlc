use std::path::Path;

use crate::core::{error::Result, types::{Document, ExtendedDocument}};

mod extractor;
mod imports;
mod validation;

pub fn resolve(path: &Path, doc: Document) -> Result<ExtendedDocument> {
    let resolved_vars = extractor::extract_vars(&doc);
    let resolve_imports = imports::resolve_imports(path, &doc)?;

    Ok(ExtendedDocument {
        nodes: doc.nodes,
        vars: resolved_vars,
        imports: resolve_imports,
    })
}
