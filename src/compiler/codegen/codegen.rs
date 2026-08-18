use crate::{
    MIN_JS_CORE,
    compiler::{
        codegen::{htmlgen::HtmlGenerator, jsgen::JsGenerator},
        obfuscation::ObfuscatedExpr,
        resolver::ExtendedDocument,
    },
    core::{
        error::{CompileError, Result},
        types::{
            Attrs, CodegenStrategy, Compiler, CompilerOptions, ComponentDocument,
            ComponentProperties, EVENT_HANDLER_ATTR_NAMES, JsBufferBuilder, Node, NodeKind,
            OutputBuffer,
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

    fn add_var(&mut self, var: &str, default_val: Option<String>) {
        let val_expr = if let Some(val) = default_val {
            parse_js_expr(&val)
        } else {
            "undefined".to_string()
        };

        self.var_zone += format!("const {}=Observable({});", var, val_expr).as_str();
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
    pub fn compile(self, compiler_options: CompilerOptions) -> Result<String> {
        let compiler = Compiler::new(compiler_options);
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
    fn new(options: CompilerOptions) -> Self {
        Self {
            buffer: OutputBuffer::new(),
            options,
            scope_id: None,
        }
    }

    fn scope_id(&mut self, id: ObfuscatedExpr) {
        self.scope_id = Some(id);
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
        compiler_options: &CompilerOptions,
    ) -> Result<String> {
        let mut sub_compiler = Compiler::new(*compiler_options);
        sub_compiler.scope_id(ObfuscatedExpr::new());

        for prop in &comp.properties {
            let ComponentProperties::Attribute(name, ..) = prop;
            sub_compiler.buffer.js.var_zone += sub_compiler.component_attributes(name).as_str();
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

        let html_string = if compiler_options.codegen_strategy == CodegenStrategy::MinifyAll {
            &sub_compiler.buffer.html.replace("> <", "><")
        } else {
            &sub_compiler.buffer.html.clone()
        };
        let js_vars = &sub_compiler.buffer.js.var_zone;
        let js_bindings = &sub_compiler.buffer.js.binding_zone;

        let nested_functions = &sub_compiler.buffer.js.component_function_zone;

        Ok(sub_compiler.component_function_declaration(
            &func_name,
            &html_string.trim(),
            &js_vars,
            &js_bindings,
            &nested_functions,
        ))
    }

    fn traverse_node(&mut self, node: &Node, edoc: &ExtendedDocument) -> Result<()> {
        let compiler_options = self.options;
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
                if let Some(scope_id) = &self.scope_id {
                    safe_attrs_map.insert("heml-comp-scope".to_string(), Some(scope_id.0.clone()));
                }

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
                        self.buffer.html += self.core_js_block(MIN_JS_CORE).as_str();
                        self.buffer.html += self.main_js_block(&self.buffer.js.build()).as_str();
                    }

                    self.buffer.html += &format!("</{}>", tag);
                }

                if let Some(id) = element_id {
                    for (event_name, event_logic) in events {
                        let js_event = &event_name[2..];
                        let logic = event_logic.unwrap_or_default();
                        self.buffer.js.binding_zone += self
                            .component_event_handling(&id, js_event, &logic)
                            .as_str();
                    }
                }
            }
            NodeKind::Raw {
                tag,
                attrs,
                content,
            } => {
                let mut content = content.clone();
                if tag.eq_ignore_ascii_case("script") {
                    self.buffer.js.binding_zone += self.user_script_block(&content).as_str();
                    return Ok(());
                } else if let Some(scope_id) = &self.scope_id && tag.eq_ignore_ascii_case("style") && attrs.exist("scoped") {
                    content = scope_css(&content, &scope_id.0);
                }
                
                self.buffer.html += &if attrs.is_empty() {
                    format!("<{}>", tag)
                } else {
                    format!("<{} {}>", tag, attrs)
                };
                self.buffer.html += content.as_str();
                self.buffer.html += &format!("</{}>", tag);
            }
            NodeKind::Text(text) => {
                self.buffer.html +=
                    if self.options.codegen_strategy == CodegenStrategy::MinifyAll {
                        minify_text(text)
                    } else {
                        text.clone()
                    }
                    .as_str();
            }
            NodeKind::Comment(comment) => {
                if self.options.codegen_strategy != CodegenStrategy::MinifyAll {
                    self.buffer.html += format!("<!--{}-->", comment).as_str()
                }
            }
            NodeKind::Value { name, fixed } => {
                if !name.starts_with('{') || !name.ends_with('}') {
                    return Err(CompileError::plain(format!(
                        "Invalid value name `{}`. Variables must be wrapped in curly braces. Did you mean `{{{}}}`?",
                        name, name
                    )));
                }

                let obfuscated_var = ObfuscatedExpr::new();

                let clean_name = if name.starts_with('{') && name.ends_with('}') {
                    &name[1..name.len() - 1]
                } else {
                    name.as_str()
                };

                let end_of_base = clean_name
                    .find(|c| c == '.' || c == '[')
                    .unwrap_or(clean_name.len());
                let base_var = &clean_name[..end_of_base];
                let remainder = &clean_name[end_of_base..];

                self.buffer.html += obfuscated_var.generate_marker().as_str();

                self.buffer.js.binding_zone += if *fixed {
                    self.fixed_value(&obfuscated_var, base_var, remainder)
                } else {
                    self.reactive_value(&obfuscated_var, base_var, remainder)
                }
                .as_str();
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
                            }
                        }
                    }

                    if !self.buffer.js.component_function_registry.contains_key(tag) {
                        let func_name = format!("comp__{}", ObfuscatedExpr::new().0);
                        self.buffer
                            .js
                            .component_function_registry
                            .insert(tag.to_string(), func_name.clone());
                        self.buffer.js.component_function_zone +=
                            &Self::generate_function_by_component_doc(
                                &comp,
                                func_name,
                                &compiler_options,
                            )?;
                    }

                    let func_name = self.buffer.js.component_function_registry.get(tag).unwrap();

                    let target_marker = ObfuscatedExpr::new();
                    self.buffer.html += &target_marker.generate_marker();

                    let mut slot_compiler = Compiler::new(compiler_options);
                    slot_compiler.traverse_nodes(edoc, children)?;

                    let mut props_js = String::from("{");
                    for (key, val) in attrs.iter() {
                        if let Some(v) = val {
                            props_js.push_str(&format!("{}:{},", key, parse_js_expr(v)));
                        } else {
                            props_js.push_str(&format!("{}:true,", key));
                        }
                    }
                    props_js.push('}');

                    let html_string = if self.options.codegen_strategy == CodegenStrategy::MinifyAll
                    {
                        slot_compiler.buffer.html.trim().replace("> <", "><")
                    } else {
                        slot_compiler.buffer.html.clone()
                    };
                    self.buffer.js.binding_zone += self
                        .component_initialization(
                            func_name,
                            &target_marker,
                            &props_js,
                            &html_string,
                            &slot_compiler.buffer.js.var_zone,
                            &slot_compiler.buffer.js.binding_zone,
                        )
                        .as_str();
                } else {
                    return Err(CompileError::plain(format!("Unkown tag `{}`", tag)));
                }
            }
            NodeKind::Slot => {
                let slot_marker = ObfuscatedExpr::new();
                self.buffer.html += &slot_marker.generate_marker();

                self.buffer.js.binding_zone += self.slot_element_replace(&slot_marker).as_str();
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

fn parse_js_expr(val: &str) -> String {
    if val.starts_with('{') && val.ends_with('}') {
        val[1..val.len() - 1].to_string()
    } else {
        format!("`{}`", val)
    }
}

fn scope_css(raw_css: &str, scope_id: &str) -> String {
    let mut scoped_css = String::new();
    
    for block in raw_css.split('}') {
        if block.trim().is_empty() { continue; }
        
        if let Some((selectors, rules)) = block.split_once('{') {
            let scoped_selectors: Vec<String> = selectors
                .split(',')
                .map(|s| format!("{}[heml-comp-scope=\"{}\"]", s.trim(), scope_id))
                .collect();
                
            scoped_css += &format!("{} {{{}}}\n", scoped_selectors.join(", "), rules);
        }
    }
    
    scoped_css
}
