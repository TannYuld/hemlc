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

    pub scope_fragment_decleration: String,
    pub var_declaration: String,
    pub component_declaration: String,
    pub js_event_element_decleration: String,
    pub js_event_binding: String,
    pub var_binding: String,
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
}

pub trait HTMLGenerator {
    fn write_doctype<'a>(out: &mut String, doctype: &'a str);
    fn write_open_tag(out: &mut String, tag: &str, physical_attrs: &PhysicalAttrs, is_void: bool);
    fn write_close_tag(out: &mut String, tag: &str);
    fn write_raw_text(out: &mut String, text: &str);

    fn inject_js(output: &mut CompilerOutput);
}

pub trait CSSGenerator {}

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

// impl TryFrom<usize> for Box<dyn minify::MinifyLevel> {
//     type Error = CompileError;

//     fn try_from(value: usize) -> Result<Self, Self::Error> {
//         match value {
//             0 => Ok(Box::new(minify::None)),
//             1 => Ok(Box::new(minify::Js)),
//             2 => Ok(Box::new(minify::All)),
//             _ => Err(CompileError::plain("Invalid minify level.")),
//         }
//     }
// }

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

            scope_fragment_decleration: String::default(),
            var_declaration: String::default(),
            component_declaration: String::default(),
            js_event_element_decleration: String::default(),
            js_event_binding: String::default(),
            var_binding: String::default(),
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
            self.scope_fragment_decleration.as_str(),
            self.var_declaration.as_str(),
            self.component_declaration.as_str(),
            self.js_event_element_decleration.as_str(),
            self.js_event_binding.as_str(),
            self.var_binding.as_str(),
            self.component_initialization.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("")
    }

    pub fn build_js(&self) -> String {
        [
            self.scope_fragment_decleration.as_str(),
            self.var_declaration.as_str(),
            self.component_declaration.as_str(),
            self.js_event_element_decleration.as_str(),
            self.js_event_binding.as_str(),
            self.var_binding.as_str(),
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
            } else {
                physical_attrs.entry(key, val_str);
            }
        }

        events
    }

    pub fn iter(&self) -> Iter<'_, HtmlEvent<'a>> {
        self.events.iter()
    }
}
