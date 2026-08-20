use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use crate::{
    compiler::resolver::{extractor, validation},
    core::{
        error::{CompileError, Result},
        lexer, parser,
        types::{ComponentDocument, ComponentProperties, Document, ExtendedDocument, Node, ResolvedImports},
    },
};

pub fn resolve_imports(path: &Path, doc: &Document) -> Result<HashMap<String, ComponentDocument>> {
    let mut imports = ResolvedImports::new();
    for node in &doc.nodes {
        _resolve_import(path, node, &mut imports)?;
    }

    Ok(imports)
}

fn _resolve_import(path: &Path, node: &Node, imports: &mut ResolvedImports) -> Result<()> {
    match &node.kind {
        crate::core::types::NodeKind::Element { tag, children, .. } => {
            if tag == "head" {
                look_for_imports(path, children, imports)?;
            } else {
                for child in children {
                    _resolve_import(path, &child, imports)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn look_for_imports(
    original_path: &Path,
    nodes: &Vec<Node>,
    imports: &mut ResolvedImports,
) -> Result<()> {
    for node in nodes {
        match &node.kind {
            crate::core::types::NodeKind::Import { src, alias } => {
                let mut path = PathBuf::from(src);
                if !path.is_absolute() {
                    path = original_path.parent().unwrap().join(path);
                }

                let comp_name = if let Some(custom_name) = alias {
                    custom_name.to_string()
                } else {
                    path.file_name().unwrap().to_str().unwrap().to_string()
                };

                imports.insert(comp_name, resolve_heml_file(path)?);
            }
            _ => {}
        }
    }
    Ok(())
}

// TODO: Improve error message here with better diagnostics.
pub fn resolve_heml_file(file_path: PathBuf) -> Result<ComponentDocument> {
    let source = std::fs::read_to_string(&file_path).map_err(|e| {
        CompileError::plain(format!(
            "Failed to read imported file at ({}): {}",
            file_path.display(),
            e
        ))
    })?;

    let tokens = lexer::tokenize(&file_path, &source)?;

    let ast: Document = parser::parse(&file_path, &source, &tokens)?;
    let mut properties: HashSet<ComponentProperties> = HashSet::new();
    let mut resolved_imports = ResolvedImports::new();
    validation::validate_and_get_properties_component_file(
        &file_path,
        &ast,
        &mut properties,
        &mut resolved_imports,
    )?;

    let resolved_vars = extractor::extract_vars(&ast);
    let edoc = ExtendedDocument {
        nodes: ast.nodes,
        vars: resolved_vars,
        imports: resolved_imports,
    };

    Ok(ComponentDocument::new(edoc, properties))
}
