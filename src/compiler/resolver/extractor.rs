use std::collections::HashSet;

use crate::core::types::{ComponentProperties, Document, JsVarMap, Node, NodeKind};

pub fn insert_component_properties(properties: &[Node], props: &mut HashSet<ComponentProperties>) {
    for property in properties {
        if let NodeKind::Attribute { name, optional } = &property.kind {
            props.insert(ComponentProperties::Attribute(name.to_string(), *optional));
        }
    }
}

pub fn extract_vars(doc: &Document) -> JsVarMap {
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
                value.as_ref().map(|defualt_val| defualt_val.to_string()),
            );
        }
        _ => {}
    }
}
