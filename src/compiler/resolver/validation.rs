use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{
    compiler::resolver::{extractor, imports},
    core::{
        error::{CompileError, Result},
        types::{ComponentProperties, Document, Node, NodeKind, ResolvedImports},
    },
};

// TOOD: Better error handling
fn validate_properties_block(nodes: &[Node]) -> Result<()> {
    for node in nodes {
        match &node.kind {
            NodeKind::Attribute { .. } | NodeKind::Text(_) | NodeKind::Comment(_) => {}

            _ => {
                return Err(CompileError::plain(
                    "Only <attribute> tags are allowed inside the <properties> block.",
                ));
            }
        }
    }
    Ok(())
}

//TODO: Better error handling
pub fn illegal_tags_check(nodes: &[Node], illegal_tags: &[&str]) -> Result<()> {
    for node in nodes {
        match &node.kind {
            NodeKind::Element { tag, children, .. } => {
                if illegal_tags
                    .iter()
                    .any(|illegal_tag| (*illegal_tag).eq_ignore_ascii_case(tag))
                {
                    return Err(CompileError::plain(
                        "Illegal tag found inside `<component>` tag",
                    ));
                }
                illegal_tags_check(children, illegal_tags)?;
            }
            NodeKind::Unknown { children, .. } => {
                illegal_tags_check(children, illegal_tags)?;
            }
            NodeKind::For { body, .. }
            | NodeKind::Data { body, .. }
            | NodeKind::Key { body, .. } => {
                illegal_tags_check(body, illegal_tags)?;
            }
            NodeKind::If {
                branches,
                otherwise,
            } => {
                for branch in branches {
                    illegal_tags_check(&branch.body, illegal_tags)?;
                }
                if let Some(other_body) = otherwise {
                    illegal_tags_check(other_body, illegal_tags)?;
                }
            }
            NodeKind::Match { arms, .. } => {
                for arm in arms {
                    illegal_tags_check(&arm.body, illegal_tags)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

//TODO: Better error handling
pub fn validate_and_get_properties_component_file(
    path: &Path,
    ast: &Document,
    comp_properties: &mut HashSet<ComponentProperties>,
    imports: &mut ResolvedImports,
) -> Result<()> {
    let control_nodes: Vec<&Node> = ast
        .nodes
        .iter()
        .filter(|node| match &node.kind {
            NodeKind::Text(t) if t.trim().is_empty() => false,
            NodeKind::Comment(_) => false,
            _ => true,
        })
        .collect();

    match control_nodes.as_slice() {
        [
            Node {
                kind: NodeKind::Doctype(doctype),
                ..
            },
            imports_slice @ ..,
            Node {
                kind: NodeKind::Properties { properties },
                ..
            },
            Node {
                kind: NodeKind::Component { childeren },
                ..
            },
        ] if doctype.eq_ignore_ascii_case("component")
            && imports_slice
                .iter()
                .all(|node| matches!(node.kind, NodeKind::Import { .. })) =>
        {
            validate_properties_block(properties)?;
            illegal_tags_check(properties, &["html", "body", "head"])?;
            illegal_tags_check(childeren, &["html", "body", "head"])?;
            extractor::insert_component_properties(properties, comp_properties);

            for import_node in imports_slice {
                if let NodeKind::Import { src, alias } = &import_node.kind {
                    let mut import_path = PathBuf::from(src);
                    if !import_path.is_absolute() {
                        import_path = path.parent().unwrap().join(import_path);
                    }

                    let tag_name = if let Some(custom_name) = alias {
                        custom_name.to_string()
                    } else {
                        import_path
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string()
                    };

                    let resolved_child = imports::resolve_heml_file(import_path)?;
                    imports.insert(tag_name, resolved_child);
                }
            }

            Ok(())
        }
        [] => Err(CompileError::plain("Component file is empty.")),
        [
            Node {
                kind: NodeKind::Doctype(doctype),
                ..
            },
            ..,
        ] if doctype != "component" => Err(CompileError::plain(
            "Component files must start with <!DOCTYPE component>",
        )),
        _ => Err(CompileError::plain(
            "Invalid component structure. A component file must contain exactly: \n\
                 1. <!DOCTYPE component>\n\
                 2. <properties>...</properties>\n\
                 3. <component>...</component>",
        )),
    }
}
