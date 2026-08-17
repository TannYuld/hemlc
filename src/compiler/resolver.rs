use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::core::{
    error::{CompileError, Result},
    lexer::tokenize,
    parser::parse,
    types::{
        ComponentDocument, ComponentProperties, Document, JsVarMap, Node, NodeKind, ResolvedImports,
    },
};

#[derive(Debug, PartialEq)]
pub struct ExtendedDocument {
    pub nodes: Vec<Node>,
    pub vars: JsVarMap,
    pub imports: ResolvedImports,
}

pub fn resolve(path: &Path, doc: Document) -> Result<ExtendedDocument> {
    let resolved_vars = extract_vars(&doc);
    let resolve_imports = resolve_imports(path, &doc)?;

    Ok(ExtendedDocument {
        nodes: doc.nodes,
        vars: resolved_vars,
        imports: resolve_imports,
    })
}

fn resolve_imports(path: &Path, doc: &Document) -> Result<HashMap<String, ComponentDocument>> {
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

//TODO: Better error handling
fn validate_and_get_properties_component_file(
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
            insert_component_properties(properties, comp_properties);

            for import_node in imports_slice {
                if let NodeKind::Import { src, alias } = &import_node.kind {
                    let mut import_path = PathBuf::from(src);
                    if !import_path.is_absolute() {
                        import_path = path.parent().unwrap().join(import_path);
                    }

                    let tag_name = if let Some(custom_name) = alias {
                        custom_name.to_string()
                    } else {
                        import_path.file_name().unwrap().to_str().unwrap().to_string()
                    };

                    let resolved_child = resolve_heml_file(import_path)?;
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

// TODO: Improve error message here with better diagnostics.
fn resolve_heml_file(file_path: PathBuf) -> Result<ComponentDocument> {
    let source = match fs::read_to_string(&file_path) {
        Ok(s) => Ok(s),
        Err(e) => Err(CompileError::plain(format!(
            "Failed to read imported file at ({}): {}",
            &file_path.display(),
            e
        ))),
    }?;

    let tokens = tokenize(&file_path, &source)?;

    let ast: Document = parse(&file_path, &source, &tokens)?;
    let mut properties: HashSet<ComponentProperties> = HashSet::new();
    let mut resolved_imports = ResolvedImports::new();
    validate_and_get_properties_component_file(&file_path, &ast, &mut properties, &mut resolved_imports)?;

    let resolved_vars = extract_vars(&ast);
    let edoc = ExtendedDocument {
        nodes: ast.nodes,
        vars: resolved_vars,
        imports: resolved_imports,
    };

    Ok(ComponentDocument::new(edoc, properties))
}

fn insert_component_properties(properties: &[Node], props: &mut HashSet<ComponentProperties>) {
    for property in properties {
        match &property.kind {
            // NodeKind::Property { name, value, body } => match name.as_str() {
            //     "PropertyPassStrategy" => {
            //         let pass_strategy = match value.as_str() {
            //             "PassAll" => Some(PropertyPassStrategy::PassAll),
            //             "PassNone" => Some(PropertyPassStrategy::PassNone),
            //             "WhiteList" => Some(PropertyPassStrategy::WhiteList(body.to_owned())),
            //             "BlackList" => Some(PropertyPassStrategy::BlackList(body.to_owned())),
            //             _ => None,
            //         };
            //         if let Some(strategy) = pass_strategy {
            //             props.insert(ComponentProperties::PropertyPassStrategy(strategy));
            //         }
            //     }
            //     _ => {}
            // },
            NodeKind::Attribute { name, optional } => {
                props.insert(ComponentProperties::Attribute(name.to_string(), *optional));
            }
            _ => {}
        }
    }
}

fn extract_vars(doc: &Document) -> JsVarMap {
    let mut vars = JsVarMap::new();
    for node in &doc.nodes {
        _extract_vars(node, &mut vars);
    }
    vars
}

fn _extract_vars(node: &Node, vars: &mut JsVarMap) {
    match &node.kind {
        crate::core::types::NodeKind::Element { children, .. } => {
            for child in children {
                _extract_vars(child, vars);
            }
        }
        NodeKind::Var { name, value } => {
            vars.insert(
                name.to_string(),
                if let Some(defualt_val) = value {
                    Some(defualt_val.to_string())
                } else {
                    None
                },
            );
        }
        _ => {}
    }
}
