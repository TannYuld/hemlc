use crate::{
    MIN_JS_CORE,
    compiler::{
        codegen::{
            HEML_ID_ATTRIBUTE_KEY, HEML_SCOPE_ATTRIBUTE_KEY, cssgen,
            htmlgen::{self, HtmlGenerator},
            jsgen::JsGenerator,
            types::{CodegenStrategy, Compiler, CompilerOptions},
            util,
        },
        obfuscation::ObfuscatedExpr,
    },
    core::{
        error::{CompileError, Result},
        types::{
            Arm, Attrs, Branch, ComponentDocument, ComponentProperties, EVENT_HANDLER_ATTR_NAMES,
            ExtendedDocument, Node, NodeKind,
        },
    },
};
use std::collections::{HashMap, HashSet};

impl Compiler {
    fn scope_id(&mut self, id: ObfuscatedExpr) {
        self.scope_id = Some(id);
    }

    pub fn compile(mut self, doc: ExtendedDocument) -> Result<String> {
        self.traverse_nodes(&doc, &doc.nodes)?;
        if self.options.codegen_strategy == CodegenStrategy::MinifyAll {
            Ok(util::minify_html_tags(&self.buffer.html))
        } else {
            Ok(self.buffer.html)
        }
    }

    fn hoist_variables(&mut self, nodes: &[Node]) {
        for node in nodes {
            match &node.kind {
                NodeKind::Var { name, .. } => {
                    self.known_observables.insert(name.to_string());
                }
                NodeKind::Element { children, .. } => {
                    self.hoist_variables(children);
                }
                _ => {}
            }
        }
    }

    fn traverse_nodes(&mut self, doc: &ExtendedDocument, nodes: &[Node]) -> Result<()> {
        self.hoist_variables(nodes);
        for node in nodes {
            self.traverse_node(node, doc)?;
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
            sub_compiler.known_observables.insert(name.to_string());
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
        sub_compiler.traverse_nodes(&comp.edoc, component_nodes)?;

        let html_string = if compiler_options.codegen_strategy == CodegenStrategy::MinifyAll {
            util::minify_html_tags(&sub_compiler.buffer.html)
        } else {
            sub_compiler.buffer.html.clone()
        };

        let js_vars = &sub_compiler.buffer.js.var_zone;
        let js_bindings = &sub_compiler.buffer.js.binding_zone;

        let nested_functions = &sub_compiler.buffer.js.component_function_zone;

        Ok(sub_compiler.component_function_declaration(
            &func_name,
            html_string.trim(),
            js_vars,
            js_bindings,
            nested_functions,
        ))
    }

    fn generate_branch_body(
        &mut self,
        nodes: &[Node],
        edoc: &ExtendedDocument,
        condition: &str,
    ) -> Result<String> {
        let mut sub_compiler = Compiler::new_subcompiler(self);

        sub_compiler.traverse_nodes(edoc, nodes)?;

        self.merge_with_subcompiler(&sub_compiler);

        let branch_html = sub_compiler.buffer.html.trim();
        let branch_vars = &sub_compiler.buffer.js.var_zone;
        let branch_bindings = &sub_compiler.buffer.js.binding_zone;

        let body = self.generate_block_body(branch_vars, branch_html, branch_bindings);

        Ok(self.generate_condition_body(condition, &body))
    }

    fn traverse_node(&mut self, node: &Node, edoc: &ExtendedDocument) -> Result<()> {
        match &node.kind {
            NodeKind::Doctype(doctype) => self.handle_doctype(doctype)?,
            NodeKind::Element {
                tag,
                attrs,
                children,
                void,
            } => self.handle_element(tag, attrs, children, *void, edoc)?,
            NodeKind::Raw {
                tag,
                attrs,
                content,
            } => self.handle_raw(tag, attrs, content)?,
            NodeKind::Text(text) => self.handle_text(text)?,
            NodeKind::Comment(comment) => self.handle_comment(comment)?,
            NodeKind::Value { name, fixed } => self.handle_value(name, *fixed)?,
            NodeKind::Unknown {
                tag,
                attrs,
                children,
            } => self.handle_unknown(tag, attrs, children, edoc)?,
            NodeKind::Slot => self.handle_slot()?,
            NodeKind::Var { name, value } => self.handle_var(name, value)?,
            NodeKind::If {
                branches,
                otherwise,
            } => self.handle_if(branches, otherwise, edoc)?,
            NodeKind::Match { value, arms } => self.handle_match(value, arms, edoc)?,
            NodeKind::For {
                each,
                binding,
                index,
                key,
                body,
            } => self.handle_for(each, binding, index, key, body, edoc)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_doctype(&mut self, doctype: &str) -> Result<()> {
        self.buffer.html += &htmlgen::generate_doctype(doctype);
        Ok(())
    }

    fn handle_element(
        &mut self,
        tag: &str,
        attrs: &Attrs,
        children: &[Node],
        void: bool,
        edoc: &ExtendedDocument,
    ) -> Result<()> {
        let mut safe_attrs_map = HashMap::new();
        if let Some(scope_id) = &self.scope_id {
            safe_attrs_map.insert(
                HEML_SCOPE_ATTRIBUTE_KEY.to_string(),
                Some(scope_id.0.clone()),
            );
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
            safe_attrs_map.insert(HEML_ID_ATTRIBUTE_KEY.to_string(), Some(id.clone()));
            Some(id)
        } else {
            None
        };

        let safe_attrs = Attrs::from_iter(
            safe_attrs_map
                .into_iter()
                .map(|(k, v)| crate::core::types::Attr::new(k, v)),
        );

        self.buffer.html += &htmlgen::build_open_tag(tag, &safe_attrs, void && children.is_empty());

        if !(void && children.is_empty()) {
            self.traverse_nodes(edoc, children)?;

            if tag == "html" {
                self.buffer.html += &self.core_js_block(MIN_JS_CORE);
                self.buffer.html += &self.main_js_block(&self.assemble_js());
            }

            self.buffer.html += &htmlgen::generate_closing_tag(tag);
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
        Ok(())
    }

    fn handle_raw(&mut self, tag: &str, attrs: &Attrs, content: &str) -> Result<()> {
        let mut content = content.to_string();
        if tag.eq_ignore_ascii_case("script") {
            let is_async = attrs.exist("async");
            self.buffer.js.binding_zone += self.user_script_block(&content, is_async).as_str();
            return Ok(());
        } else if let Some(scope_id) = &self.scope_id
            && tag.eq_ignore_ascii_case("style")
            && attrs.exist("scoped")
        {
            content = cssgen::scope_css(&content, &scope_id.0);
        } else if self.scope_id.is_none()
            && tag.eq_ignore_ascii_case("style")
            && attrs.exist("scoped")
        {
            return Err(CompileError::plain(
                "`scoped` attribute can only be used in an component document.",
            ));
        }

        self.buffer.html += &htmlgen::build_open_tag(tag, attrs, false);
        self.buffer.html += content.as_str();
        self.buffer.html += &htmlgen::generate_closing_tag(tag);
        Ok(())
    }

    fn handle_text(&mut self, text: &str) -> Result<()> {
        self.buffer.html += if self.options.codegen_strategy == CodegenStrategy::MinifyAll {
            util::minify_text(text)
        } else {
            text.to_string()
        }
        .as_str();
        Ok(())
    }

    fn handle_comment(&mut self, comment: &str) -> Result<()> {
        self.buffer.html += &self.generate_comment(comment);
        Ok(())
    }

    fn handle_value(&mut self, name: &str, fixed: bool) -> Result<()> {
        if !name.starts_with('{') || !name.ends_with('}') {
            return Err(CompileError::plain(format!(
                "Invalid value name `{}`. Variables must be wrapped in curly braces. Did you mean `{{{}}}`?",
                name, name
            )));
        }

        let expr_content = name[1..name.len() - 1].trim();
        let obfuscated_var = ObfuscatedExpr::new();

        self.buffer.html += &obfuscated_var.generate_marker();

        let final_expr = if expr_content.starts_with("() =>")
            || expr_content.starts_with("()=>")
            || expr_content.starts_with("function")
        {
            expr_content.to_string()
        } else if util::is_raw_variable(expr_content) && !self.known_locals.contains(expr_content) {
            format!("{}.value", expr_content)
        } else {
            expr_content.to_string()
        };

        let deps = util::extract_observables(&final_expr, &self.known_observables);
        let deps_array = if fixed || deps.is_empty() {
            "[]".to_string()
        } else {
            format!("[{}]", deps.join(", "))
        };

        self.buffer.js.binding_zone +=
            &self.expr_binding(&obfuscated_var, &deps_array, &final_expr);
        Ok(())
    }

    fn handle_unknown(
        &mut self,
        tag: &str,
        attrs: &Attrs,
        children: &[Node],
        edoc: &ExtendedDocument,
    ) -> Result<()> {
        if let Some(comp) = edoc.imports.get(tag) {
            for property in &comp.properties {
                match property {
                    ComponentProperties::Attribute(name, optional) => {
                        if !*optional && !attrs.exist(name) {
                            return Err(CompileError::plain("Non-optional attribute is missing."));
                        }
                    }
                }
            }
            let compiler_options = self.options;

            if !self.buffer.js.component_function_registry.contains_key(tag) {
                let func_name = format!("comp__{}", ObfuscatedExpr::new().0);
                self.buffer
                    .js
                    .component_function_registry
                    .insert(tag.to_string(), func_name.clone());
                self.buffer.js.component_function_zone +=
                    &Self::generate_function_by_component_doc(comp, func_name, &compiler_options)?;
            }

            let func_name = self.buffer.js.component_function_registry.get(tag).unwrap();

            let target_marker = ObfuscatedExpr::new();
            self.buffer.html += &target_marker.generate_marker();

            let mut slot_compiler = Compiler::new(compiler_options);
            slot_compiler.traverse_nodes(edoc, children)?;

            let mut props_js = String::from("{");
            for (key, val) in attrs.iter() {
                if let Some(v) = val {
                    props_js.push_str(&format!("{}:{},", key, util::parse_js_expr(v)));
                } else {
                    props_js.push_str(&format!("{}:true,", key));
                }
            }
            props_js.push('}');

            let html_string = if self.options.codegen_strategy == CodegenStrategy::MinifyAll {
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

        Ok(())
    }

    fn handle_slot(&mut self) -> Result<()> {
        let slot_marker = ObfuscatedExpr::new();
        self.buffer.html += &slot_marker.generate_marker();

        self.buffer.js.binding_zone += self.slot_element_replace(&slot_marker).as_str();
        Ok(())
    }

    fn handle_var(&mut self, name: &str, value: &Option<String>) -> Result<()> {
        self.buffer.js.var_zone += &self.generate_variable_decleration(name, value);
        Ok(())
    }

    fn handle_if(
        &mut self,
        branches: &[Branch],
        otherwise: &Option<Vec<Node>>,
        edoc: &ExtendedDocument,
    ) -> Result<()> {
        let expr = ObfuscatedExpr::new();

        self.buffer.html += &self.generate_conditional(&expr);

        let mut condition_bodies = String::new();
        let mut all_dependencies: HashSet<String> = HashSet::new();
        for branch in branches {
            let condition = util::parse_js_expr(&branch.condition);
            for dep in util::extract_observables(&condition, &self.known_observables) {
                all_dependencies.insert(dep);
            }
            condition_bodies += &self.generate_branch_body(&branch.body, edoc, &condition)?;
        }
        if let Some(otherwise) = otherwise {
            condition_bodies += &self.generate_branch_body(otherwise, edoc, "true")?;
        }

        let mut dependency_bindings = String::new();
        for dependecy in all_dependencies {
            dependency_bindings += &self.dependecy_binding(&dependecy, &expr);
        }

        self.buffer.js.binding_zone +=
            &self.generate_conditional_block(&expr, &condition_bodies, &dependency_bindings);
        Ok(())
    }

    fn handle_match(&mut self, value: &str, arms: &[Arm], edoc: &ExtendedDocument) -> Result<()> {
        let expr = ObfuscatedExpr::new();

        self.buffer.html += &self.generate_conditional(&expr);

        let mut condition_bodies = String::new();
        let mut all_dependencies: HashSet<String> = HashSet::new();

        let parsed_value = util::parse_js_expr(value);
        for dep in util::extract_observables(&parsed_value, &self.known_observables) {
            all_dependencies.insert(dep);
        }

        for arm in arms {
            let condition = if let Some(expr) = &arm.expr {
                format!("isMatch({}, {})", parsed_value, util::parse_js_expr(expr))
            } else {
                "true".to_string()
            };

            for dep in util::extract_observables(&condition, &self.known_observables) {
                all_dependencies.insert(dep);
            }

            condition_bodies += &self.generate_branch_body(&arm.body, edoc, &condition)?;
        }

        let mut dependency_bindings = String::new();
        for dependecy in all_dependencies {
            dependency_bindings += &self.dependecy_binding(&dependecy, &expr);
        }

        self.buffer.js.binding_zone +=
            &self.generate_conditional_block(&expr, &condition_bodies, &dependency_bindings);
        Ok(())
    }

    fn handle_for(
        &mut self,
        each: &str,
        bindings: &str,
        index: &Option<String>,
        key: &Option<String>,
        body: &[Node],
        edoc: &ExtendedDocument,
    ) -> Result<()> {
        let expr = ObfuscatedExpr::new();

        self.buffer.html += &self.generate_conditional(&expr);

        let list_observable = util::parse_js_expr(each);
        let index_var = index.clone().unwrap_or_else(|| "i".to_string());

        let key_expr = if let Some(k) = key {
            util::parse_js_expr(k)
        } else {
            index_var.clone()
        };

        let mut sub_compiler = Compiler::new_subcompiler(self);

        sub_compiler.known_observables.insert(bindings.to_string());
        sub_compiler.known_observables.insert(index_var.clone());

        sub_compiler.known_locals.insert(bindings.to_string());
        sub_compiler.known_locals.insert(index_var.clone());

        sub_compiler.traverse_nodes(edoc, body)?;

        self.merge_with_subcompiler(&sub_compiler);

        let branch_html = sub_compiler.buffer.html.trim();
        let branch_vars = &sub_compiler.buffer.js.var_zone;
        let branch_bindings = &sub_compiler.buffer.js.binding_zone;

        let render_item_body = format!(
            "
            {}
            const frag = HtmlToFragment(`{}`);
            {}
            return frag;
            ",
            branch_vars, branch_html, branch_bindings
        );

        let js_logic = format!(
            "
    const markers_{0} = FindLimitMarkers('{0}', frag);
    const update_{0} = For(
        markers_{0},
        {1},
        ({2}, {3}) => ({4}),
        ({2}, {3}) => {{
            {5}
        }}
    );
    update_{0}();
    ",
            expr.0, list_observable, bindings, index_var, key_expr, render_item_body
        );

        self.buffer.js.binding_zone += &js_logic;
        Ok(())
    }
}
