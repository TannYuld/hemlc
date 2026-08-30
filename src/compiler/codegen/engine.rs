use std::collections::HashSet;

use crate::{
    compiler::{
        codegen::{
            JS_IMPORT_REGEX,
            types::{Compiler, CompilerOutput, HtmlEventList, PhysicalAttrs, minify},
            util,
        },
        obfuscation::ObfuscatedExpr,
    },
    core::{
        error::{CompileError, Result},
        types::{Attrs, Branch, Node, NodeKind},
    },
};

impl<M: minify::MinifyLevel> Compiler<M> {
    pub fn compile(self) -> Result<String> {
        let mut output = CompilerOutput::new();
        M::write_global_scope(&mut output.scope_fragment_decleration);
        self.traverse_nodes(&self.edoc.nodes, &mut output)?;
        Ok(output.build_all())
    }

    fn traverse_nodes(&self, nodes: &[Node], output: &mut CompilerOutput) -> Result<()> {
        self.hoist_variables(nodes, output);
        for node in nodes {
            self.traverse_node(node, output)?;
        }

        Ok(())
    }

    fn hoist_variables(&self, nodes: &[Node], output: &mut CompilerOutput) {
        for node in nodes {
            match &node.kind {
                NodeKind::Var { name, .. } => {
                    output.known_observables.insert(name.to_string());
                }
                NodeKind::Element { children, .. } => {
                    self.hoist_variables(children, output);
                }
                _ => {}
            }
        }
    }

    fn traverse_node(&self, node: &Node, output: &mut CompilerOutput) -> Result<()> {
        match &node.kind {
            NodeKind::Doctype(doctype) => self.handle_doctype(doctype, output)?,
            NodeKind::Element {
                tag,
                attrs,
                children,
                void,
            } => self.handle_element(tag, attrs, children, *void, output)?,
            NodeKind::Text(text) => self.handle_text(text, output)?,
            NodeKind::Raw {
                tag,
                attrs,
                content,
            } => self.handle_raw(tag, attrs, content, output)?,
            NodeKind::Comment(comment) => self.handle_comment(comment, output)?,
            NodeKind::Value { name, fixed } => self.handle_value(name, *fixed, output)?,
            NodeKind::Var { name, value } => self.handle_var(name, value, output)?,
            NodeKind::If {
                branches,
                otherwise,
            } => self.handle_if(branches, otherwise, output)?,
            // NodeKind::For {
            //     each,
            //     binding,
            //     index,
            //     key,
            //     body,
            // } => self.handle_for(each, binding, index, key, body, edoc)?,
            // NodeKind::Match { value, arms } => self.handle_match(value, arms, edoc)?,
            // NodeKind::Unknown {
            //     tag,
            //     attrs,
            //     children,
            // } => self.handle_unknown(tag, attrs, children, edoc)?,
            // NodeKind::Slot { attrs } => self.handle_slot(attrs)?,
            _ => {}
        }
        Ok(())
    }

    fn handle_doctype(&self, doctype: &str, output: &mut CompilerOutput) -> Result<()> {
        let doctype = if doctype.eq_ignore_ascii_case("heml") {
            doctype
        } else {
            "html"
        };
        M::write_doctype(&mut output.html, doctype);
        Ok(())
    }

    fn handle_element(
        &self,
        tag: &str,
        attrs: &Attrs,
        children: &[Node],
        void: bool,
        output: &mut CompilerOutput,
    ) -> Result<()> {
        let mut physical_attrs = PhysicalAttrs::new();
        if let Some(scope) = &output.scope_id {
            physical_attrs.scope(scope.expr_ref());
        }

        let events = HtmlEventList::extract_events(attrs, &mut physical_attrs);
        let internal_element_id: String;

        if !events.is_empty() {
            internal_element_id = format!("heml_{}", ObfuscatedExpr::random());
            physical_attrs.internal_id(&internal_element_id);

            let event_list = HtmlEventList {
                id: &internal_element_id,
                events,
            };

            M::write_js_element_event(
                &mut output.js_event_binding,
                &mut output.js_event_element_decleration,
                event_list,
            );
        }
        M::write_open_tag(
            &mut output.html,
            tag,
            &physical_attrs,
            void && children.is_empty(),
        );

        if !(void && children.is_empty()) {
            self.traverse_nodes(children, output)?;

            if tag == "html" {
                M::inject_js(output);
            }

            M::write_close_tag(&mut output.html, tag);
        }
        Ok(())
    }

    fn handle_text(&self, text: &str, output: &mut CompilerOutput) -> Result<()> {
        M::write_raw_text(&mut output.html, text);
        Ok(())
    }

    fn handle_raw(
        &self,
        tag: &str,
        attrs: &Attrs,
        content: &str,
        output: &mut CompilerOutput,
    ) -> Result<()> {
        match tag {
            "script" => {
                let (js_imports, rest) = util::extract_js_imports_and_rest(content);

                M::write_user_js_imports(&mut output.user_js_import, &js_imports);
                M::write_user_js(&mut output.user_js, &rest);
            }
            "style" => {
                if let Some(scope_id) = &output.scope_id {
                    M::write_scoped_css(&mut output.html, content, scope_id.expr_ref());
                } else {
                    M::write_plain_css(&mut output.html, content);
                }
            }
            _ => {
                if attrs.exist("scoped") {
                    return Err(CompileError::plain(
                        "`scoped` attribute can only be used in an component document.",
                    ));
                }
                self.handle_element(tag, attrs, &vec![], false, output)?;
            }
        }

        Ok(())
    }

    fn handle_comment(&self, comment: &str, output: &mut CompilerOutput) -> Result<()> {
        M::write_comment(&mut output.html, comment);
        Ok(())
    }

    fn handle_value(&self, name: &str, fixed: bool, output: &mut CompilerOutput) -> Result<()> {
        if !name.starts_with('{') || !name.ends_with('}') {
            return Err(CompileError::plain(format!(
                "Invalid value name `{}`. Variables must be wrapped in curly braces. Did you mean `{{{}}}`?",
                name, name
            )));
        }

        let expr_content = name[1..name.len() - 1].trim();
        let obfuscated_var = ObfuscatedExpr::new();

        let final_expr = if expr_content.starts_with("() =>")
            || expr_content.starts_with("()=>")
            || expr_content.starts_with("function")
        {
            expr_content.to_string()
        } else if util::is_raw_variable(expr_content) && !output.known_locals.contains(expr_content)
        {
            format!("{}.value", expr_content)
        } else {
            expr_content.to_string()
        };

        let deps = util::extract_observables(&final_expr, &output.known_observables);
        let deps_array = if fixed || deps.is_empty() {
            "[]".to_string()
        } else {
            format!("[{}]", deps.join(", "))
        };

        M::write_marker(&mut output.html, &obfuscated_var);
        M::write_expr_bind(
            &mut output.expr_binding,
            obfuscated_var.expr_ref(),
            &deps_array,
            &final_expr,
        );

        Ok(())
    }

    fn handle_var(
        &self,
        name: &str,
        value: &Option<String>,
        output: &mut CompilerOutput,
    ) -> Result<()> {
        M::write_var_declaration(&mut output.var_declaration, name, value);
        Ok(())
    }

    fn handle_if(
        &self,
        branches: &[Branch],
        otherwise: &Option<Vec<Node>>,
        output: &mut CompilerOutput,
    ) -> Result<()> {
        let expr = ObfuscatedExpr::new();

        let mut condition_bodies = String::new();
        let mut all_dependencies: HashSet<String> = HashSet::new();
        for branch in branches {
            let condition = util::parse_js_expr(&branch.condition);
            for dep in util::extract_observables(&condition, &output.known_observables) {
                all_dependencies.insert(dep);
            }
            condition_bodies += &self.generate_branch_body(&branch.body, &condition)?;
        }
        if let Some(otherwise) = otherwise {
            condition_bodies += &self.generate_branch_body(otherwise, "true")?;
        }

        println!("\n[YOLO]\n{}", condition_bodies);
        // let mut dependency_bindings = String::new();
        for dependecy in all_dependencies {
            M::write_dependecy_binding(&mut output.expr_binding, &dependecy, expr.expr_ref());
        }

        Ok(())
    }

    fn generate_branch_body(&self, nodes: &[Node], condition: &str) -> Result<String> {
        let mut inner_output = CompilerOutput::new().with_scope();

        self.traverse_nodes(nodes, &mut inner_output)?;

        Ok(M::generate_conditional_branch_body(
            &inner_output.build_all(),
            &inner_output.minified_js_build(),
            condition,
            &inner_output.var_declaration,
            &inner_output.expr_binding,
        ))
    }
}
