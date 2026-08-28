use std::{
    collections::{HashMap, HashSet},
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
    pub js_event_binding: String,
    pub var_binding: String,
    pub component_initialization: String,

    pub known_observables: HashSet<String>,
    pub known_locals: HashSet<String>,
    pub scope_id: Option<ObfuscatedExpr>,
}

pub trait JSGenerator {
    fn write_js_element_event(out: &mut String, events: HtmlEventList, id: &str);
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
            self.js_event_binding.as_str(),
            self.var_binding.as_str(),
            self.component_initialization.as_str(),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
    }
}

pub struct PhysicalAttrs<'a>(HashMap<&'a str, Option<&'a str>>);

impl<'a> PhysicalAttrs<'a> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn scope(&mut self, scope_id: &'a str) {
        self.0.insert(HEML_SCOPE_ATTRIBUTE_KEY, Some(scope_id));
    }

    pub fn entry(&mut self, key: &'a str, value: Option<&'a str>) {
        self.0.insert(key, value);
    }

    pub fn internal_id(&mut self, id: &'a str) {
        self.0.insert(HEML_ID_ATTRIBUTE_KEY, Some(id));
    }

    pub fn has_attr(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn merge(&mut self, attrs: &'a Attrs) {
        for (key, val) in attrs.iter() {
            if self.0.contains_key(key.as_str()) {
                continue;
            }
            self.entry(key, val.as_deref());
        }
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

pub struct HtmlEventList<'a>(Vec<(&'a str, &'a str, bool)>);

impl<'a> HtmlEventList<'a> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn load_events_if_not<F>(&mut self, attrs: &'a Attrs, mut if_not_callback: F)
    where
        F: FnMut(&'a str, Option<&'a str>),
    {
        for (key, val) in attrs.iter() {
            if let Some(val) = val {
                let event_body = if val.starts_with("{") && val.ends_with("}") {
                    &val[1..val.len() - 1]
                } else {
                    val
                };

                if EVENT_HANDLER_ATTR_NAMES
                    .iter()
                    .any(|event| event.eq_ignore_ascii_case(key))
                {
                    self.0.push((&key[2..], event_body, false));
                } else if key.starts_with("async:")
                    && EVENT_HANDLER_ATTR_NAMES
                        .iter()
                        .any(|event| event.eq_ignore_ascii_case(&key[6..]))
                {
                    self.0.push((&key[8..], event_body, true));
                }
            } else {
                if_not_callback(key, val.as_deref());
            }
        }
    }

    pub fn has_event(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn iter(&self) -> Iter<'a, (&str, &str, bool)> {
        self.0.iter()
    }
}
