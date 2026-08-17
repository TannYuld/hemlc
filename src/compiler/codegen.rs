use crate::{
    MIN_JS_CORE,
    compiler::{obfuscation::ObfuscatedExpr, resolver::ExtendedDocument},
    core::{
        error::{CompileError, Result},
        types::{
            Attrs, Compiler, ComponentDocument, ComponentProperties, EVENT_HANDLER_ATTR_NAMES,
            JsBufferBuilder, Node, NodeKind, OutputBuffer,
        },
    },
};
use std::{collections::HashMap, fmt::Display};

impl OutputBuffer {
    fn new() -> Self {
        Self {
            js: JsBufferBuilder::new(),
            html: String::new(),
        }
    }

    fn build(self) -> String {
        self.html
    }
}

impl JsBufferBuilder {
    fn new() -> Self {
        Self {
            var_zone: String::new(),
            binding_zone: String::new(),
            component_function_zone: String::new(),
            component_function_registry: HashMap::new(),
        }
    }

    fn add_var(&mut self, var: &str, defualt_val: Option<String>) {
        self.var_zone += format!(
            "const {}=Observable({});",
            var,
            if let Some(val) = defualt_val {
                val
            } else {
                String::new()
            }
        )
        .as_str();
    }

    fn add_var_binding(&mut self, obfuscated_marker: &ObfuscatedExpr, var_name: &str) {
        self.binding_zone += &format!(
            "const marker__{0}=FindMarker(\"{0}\",frag);BindValue(marker__{0},{1});",
            obfuscated_marker.0, var_name
        );
    }

    fn build(&self) -> String {
        let mut result_buffer = String::new();
        result_buffer += &self.component_function_zone;
        result_buffer += &self.var_zone;
        result_buffer += "const frag=document;";
        result_buffer += &self.binding_zone;

        result_buffer
    }
}

impl ExtendedDocument {
    pub fn compile(self) -> Result<String> {
        let compiler = Compiler::new();
        Ok(compiler.compile(&self)?)
    }
}

impl Display for Attrs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buffer = String::new();
        for (key, val) in self.iter() {
            buffer += key.as_str();
            if let Some(val) = val {
                buffer += format!("=\"{}\" ", val.as_str()).as_str();
            } else {
                buffer += " ";
            }
        }
        f.write_str(buffer.trim_end())
    }
}

impl Compiler {
    fn new() -> Self {
        Self {
            buffer: OutputBuffer::new(),
        }
    }

    fn compile(mut self, doc: &ExtendedDocument) -> Result<String> {
        self.traverse_nodes(doc, &doc.nodes)?;
        Ok(self.buffer.build())
    }

    fn traverse_nodes(&mut self, doc: &ExtendedDocument, nodes: &[Node]) -> Result<()> {
        for node in nodes {
            self.traverse_node(node, &doc)?;
        }
        Ok(())
    }

    fn generate_function_by_component_doc(
        comp: &ComponentDocument,
        func_name: String,
    ) -> Result<String> {
        let mut sub_compiler = Compiler::new();

        for prop in &comp.properties {
            let ComponentProperties::Attribute(name, ..) = prop;
            sub_compiler.buffer.js.var_zone += &format!(
                "const {0}=(props.{0}!==undefined&&props.{0}.addSubscriber)?props.{0}:Observable(props.{0});",
                name
            );
        }

        let component_nodes = comp
            .edoc
            .nodes
            .iter()
            .find_map(|node| {
                if let NodeKind::Component { childeren } = &node.kind {
                    Some(childeren.as_slice())
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                CompileError::plain("Invalid component file: missing `<component>` block.")
            })?;
        sub_compiler.traverse_nodes(&comp.edoc, &component_nodes)?;

        let html_string = sub_compiler.buffer.html.replace("> <", "><");
        let js_vars = sub_compiler.buffer.js.var_zone;
        let js_bindings = sub_compiler.buffer.js.binding_zone;

        let nested_functions = sub_compiler.buffer.js.component_function_zone;

        let func_code = format!(
            "{}function {}(marker_target,props,slot_frag){{{}const frag=HtmlToFragment(`{}`);{}marker_target.after(frag);}}",
            nested_functions,
            func_name,
            js_vars,
            html_string.trim(),
            js_bindings
        );

        Ok(func_code)
    }

    fn traverse_node(&mut self, node: &Node, edoc: &ExtendedDocument) -> Result<()> {
        match &node.kind {
            NodeKind::Doctype(doctype) => {
                self.buffer.html += format!("<!DOCTYPE {}>", doctype).as_str();
            }
            NodeKind::Element {
                tag,
                attrs,
                children,
                void,
            } => {
                let mut safe_attrs_map = HashMap::new();
                let mut events = Vec::new();
                for (key, val) in attrs.iter() {
                    let key_lower = key.to_ascii_lowercase();

                    if EVENT_HANDLER_ATTR_NAMES.contains(&key_lower.as_str()) {
                        events.push((key_lower, val.clone()));
                    } else {
                        safe_attrs_map.insert(key.clone(), val.clone());
                    }
                }

                let element_id = if !events.is_empty() {
                    let id = format!("heml_{}", ObfuscatedExpr::new().0);
                    safe_attrs_map.insert("data-heml-id".to_string(), Some(id.clone()));
                    Some(id)
                } else {
                    None
                };

                let safe_attrs = Attrs::from_iter(
                    safe_attrs_map
                        .into_iter()
                        .map(|(k, v)| crate::core::types::Attr::new(k, v)),
                );

                self.buffer.html += &build_open_tag(tag, &safe_attrs, *void && children.is_empty());

                if !(*void && children.is_empty()) {
                    self.traverse_nodes(edoc, children)?;

                    if tag == "html" {
                        self.buffer.html += &format!(
                            "\n<!-- Injected core.min.js - for basic reactive utility -->\n<script>\n{}\n</script>\n",
                            MIN_JS_CORE
                        );
                        self.buffer.html += &format!(
                            "\n<!-- Autogenerated compiled JS utility for reactivity -->\n<script>\n{}\n</script>\n",
                            self.buffer.js.build()
                        );
                    }

                    self.buffer.html += &format!("</{}>", tag);
                }

                if let Some(id) = element_id {
                    for (event_name, event_logic) in events {
                        let js_event = &event_name[2..];
                        let logic = event_logic.unwrap_or_default();

                        self.buffer.js.binding_zone += &format!(
                            "frag.querySelector('[data-heml-id=\"{}\"]').addEventListener('{}',async (event)=>{{{}}});",
                            id, js_event, logic
                        );
                    }
                }
            }
            NodeKind::Raw {
                tag,
                attrs,
                content,
            } => {
                if tag.eq_ignore_ascii_case("script") {
                    self.buffer.js.binding_zone += &format!(
                        "((document) => {{{}}})(new Proxy(document,{{get(target,prop){{if(prop==='getElementById'){{return (id)=>frag.querySelector(`[id=\"${{id}}\"]`);}}if(prop==='querySelector'||prop==='querySelectorAll'){{return frag[prop].bind(frag);}}const val=target[prop];return typeof val==='function'?val.bind(target):val;}}}}));",
                        content
                    );
                } else {
                    self.buffer.html += &if attrs.is_empty() {
                        format!("<{}>", tag)
                    } else {
                        format!("<{} {}>", tag, attrs)
                    };
                    self.buffer.html += content.as_str();
                    self.buffer.html += &format!("</{}>", tag);
                }
            }
            NodeKind::Text(text) => {
                let minified = minify_text(text);
                self.buffer.html += minified.as_str();
            }
            NodeKind::Comment(..) => {
                // self.buffer.html += format!("<!--{}-->", comment).as_str()
            }
            NodeKind::Value { name, fixed } => {
                let obfuscated_var = ObfuscatedExpr::new();
                if *fixed {
                    if let Some(var) = edoc.vars.get(name)
                        && let Some(var_val) = var
                    {
                        self.buffer.html += var_val;
                    }
                } else {
                    self.buffer.html += obfuscated_var.generate_marker().as_str();
                    self.buffer
                        .js
                        .add_var_binding(&obfuscated_var, name.as_str());
                }
            }
            NodeKind::Unknown {
                tag,
                attrs,
                children,
            } => {
                if let Some(comp) = edoc.imports.get(tag) {
                    for property in &comp.properties {
                        match property {
                            ComponentProperties::Attribute(name, optional) => {
                                if !*optional && !attrs.exist(name) {
                                    return Err(CompileError::plain(
                                        "Non-optional attribute is missing.",
                                    ));
                                }
                            } // ComponentProperties::PropertyPassStrategy(property_pass_strategy) => {
                              //     match property_pass_strategy {
                              //         crate::lib::types::PropertyPassStrategy::WhiteList(items) => {
                              //             for (attr_key, _) in attrs.iter() {
                              //                 if !items.contains(attr_key) {
                              //                     return Err(CompileError::plain(
                              //                         "Illegal attribute passed to component.",
                              //                     ));
                              //                 }
                              //             }
                              //         }
                              //         crate::lib::types::PropertyPassStrategy::BlackList(items) => {
                              //             for (attr_key, _) in attrs.iter() {
                              //                 if items.contains(attr_key) {
                              //                     return Err(CompileError::plain(
                              //                         "Illegal attribute passed to component.",
                              //                     ));
                              //                 }
                              //             }
                              //         }
                              //         crate::lib::types::PropertyPassStrategy::PassNone => {
                              //             if !attrs.is_empty() {
                              //                 return Err(CompileError::plain(
                              //                     "Attributes passed to component wiwth `PassNone` rule.",
                              //                 ));
                              //             }
                              //         }
                              //         _ => {}
                              //     }
                              // }
                        }
                    }

                    if !self.buffer.js.component_function_registry.contains_key(tag) {
                        let func_name = format!("comp__{}", ObfuscatedExpr::new().0);
                        self.buffer
                            .js
                            .component_function_registry
                            .insert(tag.to_string(), func_name.clone());
                        self.buffer.js.component_function_zone +=
                            &Self::generate_function_by_component_doc(&comp, func_name)?;
                    }

                    let func_name = self.buffer.js.component_function_registry.get(tag).unwrap();

                    let target_marker = ObfuscatedExpr::new();
                    self.buffer.html += &target_marker.generate_marker();

                    let mut slot_compiler = Compiler::new();
                    slot_compiler.traverse_nodes(edoc, children)?;

                    let minified_slot_html = slot_compiler.buffer.html.trim().replace("> <", "><");

                    self.buffer.js.binding_zone += &format!(
                        "const target_{0}=FindMarker('{0}',frag);const slot_frag_{0}=(()=>{{{1}const frag=HtmlToFragment(`{2}`);{3}return frag;}})();{4}(target_{0},{5},slot_frag_{0});",
                        target_marker.0,
                        slot_compiler.buffer.js.var_zone,
                        minified_slot_html,
                        slot_compiler.buffer.js.binding_zone,
                        func_name,
                        attrs.to_json()
                    );
                } else {
                    return Err(CompileError::plain(format!("Unkown tag `{}`", tag)));
                }
            }
            NodeKind::Slot => {
                let slot_marker = ObfuscatedExpr::new();

                self.buffer.html += &slot_marker.generate_marker();

                self.buffer.js.binding_zone += &format!(
                    "const m_{0}=FindMarker('{0}',frag);if(slot_frag){{m_{0}.replaceWith(slot_frag);}}else{{m_{0}.remove();}}",
                    slot_marker.0
                );
            }
            NodeKind::Var { name, value } => {
                self.buffer.js.add_var(name, value.to_owned());
            }
            _ => {}
        }
        Ok(())
    }
}

fn build_open_tag(tag: &str, attrs: &Attrs, is_void: bool) -> String {
    if attrs.is_empty() {
        if is_void {
            format!("<{}/>", tag)
        } else {
            format!("<{}>", tag)
        }
    } else {
        if is_void {
            format!("<{} {}/>", tag, attrs)
        } else {
            format!("<{} {}>", tag, attrs)
        }
    }
}

fn minify_text(text: &str) -> String {
    let words: Vec<&str> = text.split_ascii_whitespace().collect();

    if words.is_empty() {
        return " ".to_string();
    }

    let mut minified = words.join(" ");

    if text
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_whitespace())
    {
        minified.insert(0, ' ');
    }

    if text
        .chars()
        .last()
        .map_or(false, |c| c.is_ascii_whitespace())
    {
        minified.push(' ');
    }

    minified
}
