use std::{
    collections::{HashMap, HashSet, hash_map},
    fmt::Display,
    marker::PhantomData,
    slice::Iter,
};

use indoc::indoc;

use crate::{
    compiler::{
        codegen::{HEML_ID_ATTRIBUTE_KEY, HEML_SCOPE_ATTRIBUTE_KEY},
        obfuscation::ObfuscatedExpr,
    },
    core::{
        error::{self, CompileError},
        types::{Attrs, EVENT_HANDLER_ATTR_NAMES, ExtendedDocument},
    },
};

#[derive(Debug, Clone)]
pub struct CompilerOutput {
    pub html: String,

    pub user_js_import: String,
    pub scope_fragment_decleration: String,
    pub var_declaration: String,
    pub component_declaration: String,
    pub user_js: String,
    pub js_event_element_decleration: String,
    pub js_event_binding: String,
    pub expr_binding: String,
    pub component_initialization: String,

    pub known_observables: HashSet<String>,
    pub known_locals: HashSet<String>,
    pub scope_id: Option<ObfuscatedExpr>,
}

pub trait JSGenerator {
    fn write_js_element_event(
        out_event_binding: &mut String,
        out_event_element_decleration: &mut String,
        events: HtmlEventList,
    );
    fn write_global_scope(out: &mut String);

    fn write_expr_bind(out: &mut String, var: &str, deps_array: &str, expr: &str);
    fn write_var_declaration(out: &mut String, var: &str, value: &Option<String>);
    fn write_dependecy_binding(out: &mut String, dependency: &str, expr: &str);

    fn write_if_statment(out: &mut String, expr: &str, branch_bodies: &str);
    fn generate_conditional_branch_body(html: &str, js: &str, condition: &str, vars: &str, bindings: &str,) -> String;
}

pub trait HTMLGenerator {
    fn write_doctype<'a>(out: &mut String, doctype: &'a str);
    fn write_open_tag(out: &mut String, tag: &str, physical_attrs: &PhysicalAttrs, is_void: bool);
    fn write_close_tag(out: &mut String, tag: &str);
    fn write_raw_text(out: &mut String, text: &str);

    fn write_comment(out: &mut String, comment: &str);
    fn write_marker(out: &mut String, marker: &ObfuscatedExpr);
    fn write_marker_pairs(out: &mut String, marker: &ObfuscatedExpr);

    fn write_user_js_imports(out: &mut String, js: &str);
    fn write_user_js(out: &mut String, js: &str);

    fn write_plain_css(out: &mut String, css: &str);
    fn write_scoped_css(out: &mut String, css: &str, scope_id: &str);

    fn inject_js(output: &mut CompilerOutput);
}

pub trait CSSGenerator {
    // fn write_plain
}

pub mod minify {
    use crate::compiler::codegen::types::{CSSGenerator, HTMLGenerator, JSGenerator};

    pub struct None;
    pub struct Js;
    pub struct All;

    pub trait MinifyLevel: HTMLGenerator + JSGenerator + CSSGenerator {}

    impl MinifyLevel for None {}
    impl MinifyLevel for Js {}
    impl MinifyLevel for All {}
}

pub struct Compiler<M>
where
    M: minify::MinifyLevel,
{
    pub edoc: ExtendedDocument,
    _phantom_data: PhantomData<M>,
}

impl<M: minify::MinifyLevel> Compiler<M> {
    pub fn new(edoc: ExtendedDocument) -> Self {
        Self {
            edoc,
            _phantom_data: PhantomData,
        }
    }
}

impl CompilerOutput {
    pub fn new() -> Self {
        Self {
            html: String::default(),

            user_js_import: String::default(),
            scope_fragment_decleration: String::default(),
            var_declaration: String::default(),
            component_declaration: String::default(),
            user_js: String::default(),
            js_event_element_decleration: String::default(),
            js_event_binding: String::default(),
            expr_binding: String::default(),
            component_initialization: String::default(),

            known_observables: HashSet::default(),
            known_locals: HashSet::default(),
            scope_id: None,
        }
    }

    pub fn with_scope(self) -> Self {
        let mut _self = self;
        _self.scope_id = Some(ObfuscatedExpr::new());
        _self
    }

    pub fn build_all(&self) -> String {
        self.html.clone()
    }

    pub fn minified_js_build(&self) -> String {
        [
            self.user_js_import.as_str(),
            self.scope_fragment_decleration.as_str(),
            self.var_declaration.as_str(),
            self.component_declaration.as_str(),
            self.user_js.as_str(),
            self.js_event_element_decleration.as_str(),
            self.js_event_binding.as_str(),
            self.expr_binding.as_str(),
            self.component_initialization.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("")
    }

    pub fn build_js(&self) -> String {
        [
            self.user_js_import.as_str(),
            self.scope_fragment_decleration.as_str(),
            self.var_declaration.as_str(),
            self.component_declaration.as_str(),
            self.user_js.as_str(),
            self.js_event_element_decleration.as_str(),
            self.js_event_binding.as_str(),
            self.expr_binding.as_str(),
            self.component_initialization.as_str(),
        ]
        .into_iter()
        .map(|s| s.trim_matches(['\r', '\n']))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
    }
}

#[derive(Debug)]
pub struct PhysicalAttrs<'a>(Vec<(&'a str, Option<&'a str>)>);

impl<'a> PhysicalAttrs<'a> {
    pub fn new() -> Self {
        Self(Vec::with_capacity(4))
    }

    pub fn entry(&mut self, key: &'a str, value: Option<&'a str>) {
        self.0.push((key, value));
    }

    pub fn scope(&mut self, scope_id: &'a str) {
        self.entry(HEML_SCOPE_ATTRIBUTE_KEY, Some(scope_id));
    }

    pub fn internal_id(&mut self, id: &'a str) {
        self.entry(HEML_ID_ATTRIBUTE_KEY, Some(id));
    }

    pub fn has_attrs(&self) -> bool {
        !self.0.is_empty()
    }
}

impl Display for PhysicalAttrs<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, (key, value)) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", key)?;
            if let Some(val) = value {
                write!(f, "=\"{}\"", val)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct HtmlEvent<'a> {
    pub event_name: &'a str,
    pub event_body: &'a str,
    pub is_async: bool,
}

impl<'a> HtmlEvent<'a> {
    pub fn new(event_name: &'a str, event_body: &'a str, is_async: bool) -> Self {
        Self {
            event_name,
            event_body,
            is_async,
        }
    }
}

pub struct HtmlEventList<'a> {
    pub id: &'a str,
    pub events: Vec<HtmlEvent<'a>>,
}

impl<'a> HtmlEventList<'a> {
    pub fn extract_events(
        attrs: &'a Attrs,
        physical_attrs: &mut PhysicalAttrs<'a>,
    ) -> Vec<HtmlEvent<'a>> {
        let mut events = Vec::new();

        for (key, val) in attrs.iter() {
            let val_str = val.as_deref();
            let event_body = val_str
                .map(|v| {
                    if v.starts_with('{') && v.ends_with('}') {
                        &v[1..v.len() - 1]
                    } else {
                        v
                    }
                })
                .unwrap_or_default();

            let key_lower = key.to_ascii_lowercase();

            if EVENT_HANDLER_ATTR_NAMES.iter().any(|e| e.eq_ignore_ascii_case(&key_lower)) {
                events.push(HtmlEvent::new(&key[2..], event_body, false));
            } else if key_lower.starts_with("async:")
                && EVENT_HANDLER_ATTR_NAMES.iter().any(|e| e.eq_ignore_ascii_case(&key_lower[6..]))
            {
                events.push(HtmlEvent::new(&key[8..], event_body, true));
            } else if key != HEML_ID_ATTRIBUTE_KEY && key != HEML_SCOPE_ATTRIBUTE_KEY {
                physical_attrs.entry(key, val_str);
            }
        }

        events
    }

    pub fn iter(&self) -> Iter<'_, HtmlEvent<'a>> {
        self.events.iter()
    }
}
