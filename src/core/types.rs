use std::collections::{HashMap, HashSet};

use crate::{compiler::{obfuscation::ObfuscatedExpr, resolver::ExtendedDocument}, core::error::CompileError};

#[derive(Debug, PartialEq, Clone)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub start: usize,
}

impl<'a> Token<'a> {
    pub fn new(kind: TokenKind<'a>, start: usize) -> Token<'a> {
        Token { kind, start }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind<'a> {
    TagOpen {
        name: &'a str,
        attrs: Vec<(&'a str, Option<&'a str>)>,
        self_closing: bool,
    },
    TagClose(&'a str),
    Text(&'a str),
    Comment(&'a str),
    /// `<!DOCTYPE html>` -> Doctype("html")
    Doctype(&'a str),
    /// Contents of a raw-text element (`script`, `style`), never tokenized further.
    RawText(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Attrs(HashMap<String, Option<String>>);

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Attr {
    pub key: String,
    pub value: Option<String>,
}

impl Attr {
    pub fn new(key: impl Into<String>, value: Option<impl Into<String>>) -> Self {
        Attr {
            key: key.into(),
            value: if let Some(val) = value {
                Some(val.into())
            } else {
                None
            },
        }
    }
}

impl Attrs {
    pub fn attr<'a>(&'a self, key: &str) -> Option<&'a str> {
        if let Some(val) = self.0.get(key) {
            if let Some(val) = val {
                return Some(val.as_str());
            }
        }
        None
    }

    pub fn exist(&self, key: &str) -> bool {
        self.0.get(key).is_some()
    }

    pub fn iter(&self) -> std::collections::hash_map::Iter<'_, String, Option<String>> {
        self.0.iter()
    }

    pub fn empty() -> Attrs {
        Attrs(HashMap::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FromIterator<Attr> for Attrs {
    fn from_iter<T: IntoIterator<Item = Attr>>(iter: T) -> Self {
        let mut _self = Self::empty();
        for attr in iter {
            _self.0.insert(attr.key, attr.value);
        }

        _self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub start: usize,
}

impl Node {
    pub fn new(kind: NodeKind, start: usize) -> Self {
        Node { kind, start }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    // ---- plain markup -----------------------------------------------------
    Element {
        tag: String,
        attrs: Attrs,
        children: Vec<Node>,
        void: bool,
    },
    /// `script` / `style`: body is opaque text handed through untouched.
    Raw {
        tag: String,
        attrs: Attrs,
        content: String,
    },
    Text(String),
    Comment(String),
    Doctype(String),



    // ---- HEML -------------------------------------------------------------
    /// `<import src="./mybutton.html" as="mybutton"/>`
    Import {
        src: String,
        alias: Option<String>,
    },
    /// `<var name="x" value="0"/>` -> an Observable in the generated script.
    Var {
        name: String,
        value: Option<String>,
    },
    /// `<if>/<elseif>/<else>` collapsed into one node by the parser.
    If {
        branches: Vec<Branch>,
        otherwise: Option<Vec<Node>>,
    },
    /// `<match value="p"><arm expr="1">..</arm><arm default>..</arm></match>`
    Match {
        value: String,
        arms: Vec<Arm>,
    },
    /// `<for each="items" as="item" index="i" key=".id">..</for>`
    For {
        each: String,
        binding: String,
        index: Option<String>,
        key: Option<String>,
        body: Vec<Node>,
    },
    /// `<value name="result" fixed>`
    /// With optional fixed attribute, it doesnt subscribe rerender for the value changes.
    Value {
        name: String,
        fixed: bool,
    },
    /// `<data name="result">..</data>`: opens a data scope.
    /// With no children it dumps the value as JSON.
    Data {
        path: String,
        body: Vec<Node>,
    },
    /// `<key name="person">..</key>`: walks into the current data scope.
    Key {
        path: String,
        body: Vec<Node>,
    },
    // ---- HEML - Component -----------------------------------------------------
    /// `<children/>` inside a component definition.
    Slot,
    // Property {
    //     name: String,
    //     value: String,
    //     body: Vec<String>,
    // },
    Attribute {
        name: String,
        optional: bool
    },
    Properties {
        properties: Vec<Node>,
    },
    Component {
        childeren: Vec<Node>,
    },

    Unknown {
        tag: String,
        attrs: Attrs,
        children: Vec<Node>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub condition: String,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    /// `None` for the default arm.
    pub expr: Option<String>,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub nodes: Vec<Node>,
}

// Code generation phase /////////////////////

pub struct JsBufferBuilder {
    pub var_zone: String,
    pub binding_zone: String,
    pub component_function_zone: String,
    pub component_function_registry: HashMap<String, String>,
}

pub struct OutputBuffer {
    pub js: JsBufferBuilder,
    pub html: String,
}

pub struct Compiler {
    pub buffer: OutputBuffer,
    pub options: CompilerOptions,
    pub scope_id: Option<ObfuscatedExpr>
}

#[derive(Debug, Clone, Copy)]
pub struct CompilerOptions{
    pub codegen_strategy: CodegenStrategy
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodegenStrategy {
    AsIs,
    MinifyJsOnly,
    MinifyAll
}

#[derive(Debug, PartialEq)]
pub struct ComponentDocument {
    pub edoc: ExtendedDocument,
    pub properties: HashSet<ComponentProperties>,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum ComponentProperties {
    // PropertyPassStrategy(PropertyPassStrategy),
    Attribute(String, bool)
}

// #[derive(Debug, Hash, PartialEq, Eq)]
// pub enum PropertyPassStrategy {
//     WhiteList(Vec<String>),
//     BlackList(Vec<String>),
//     PassAll,
//     PassNone,
// }

pub type JsVarMap = HashMap<String, Option<String>>;
pub type ResolvedImports = HashMap<String, ComponentDocument>;

impl ComponentDocument {
    pub fn new(edoc: ExtendedDocument, properties: HashSet<ComponentProperties>) -> Self {
        Self { edoc, properties }
    }
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self { codegen_strategy: CodegenStrategy::MinifyAll }
    }
}

impl TryFrom<usize> for CodegenStrategy {
    type Error = CompileError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::AsIs),
            1 => Ok(Self::MinifyJsOnly),
            2 => Ok(Self::MinifyAll),
            _ => Err(CompileError::plain("Invalid minify level."))
        }
    }
}

// ---------------------------------------------------------------------------
// Tag classification
#[allow(dead_code)]
pub const HEML_TAGS: &[&str] = &[
    "import",
    "component",
    "properties",
    "property",
    "childeren",
    "var",
    "if",
    "elseif",
    "else",
    "match",
    "arm",
    "for",
    "value",
    "data",
    "key",
    "children",
    "component",
    "properties",
    "prop",
];

pub const EVENT_HANDLER_ATTR_NAMES: &[&'static str] = &[
    "onabort",
    "onauxclick",
    "onbeforeinput",
    "onbeforematch",
    "onbeforetoggle",
    "onblur",
    "oncancel",
    "oncanplay",
    "oncanplaythrough",
    "onchange",
    "onclick",
    "onclose",
    "oncontextlost",
    "oncontextmenu",
    "oncontextrestored",
    "oncopy",
    "oncuechange",
    "oncut",
    "ondblclick",
    "ondrag",
    "ondragend",
    "ondragenter",
    "ondragleave",
    "ondragover",
    "ondragstart",
    "ondrop",
    "ondurationchange",
    "onemptied",
    "onended",
    "onerror",
    "onfocus",
    "onformdata",
    "onfullscreenchange",
    "onfullscreenerror",
    "ongotpointercapture",
    "oninput",
    "oninvalid",
    "onkeydown",
    "onkeyup",
    "onload",
    "onloadeddata",
    "onloadedmetadata",
    "onloadstart",
    "onlostpointercapture",
    "onmousedown",
    "onmouseenter",
    "onmouseleave",
    "onmousemove",
    "onmouseout",
    "onmouseover",
    "onmouseup",
    "onpaste",
    "onpause",
    "onplay",
    "onplaying",
    "onpointercancel",
    "onpointerdown",
    "onpointerenter",
    "onpointerleave",
    "onpointermove",
    "onpointerout",
    "onpointerover",
    "onpointerrawupdate",
    "onpointerup",
    "onprogress",
    "onratechange",
    "onreset",
    "onresize",
    "onscroll",
    "onscrollend",
    "onsecuritypolicyviolation",
    "onseeked",
    "onseeking",
    "onselect",
    "onslotchange",
    "onstalled",
    "onsubmit",
    "onsuspend",
    "ontimeupdate",
    "ontoggle",
    "ontouchcancel",
    "ontouchend",
    "ontouchmove",
    "ontouchstart",
    "ontransitioncancel",
    "ontransitionend",
    "ontransitionrun",
    "ontransitionstart",
    "onvolumechange",
    "onwaiting",
    "onwebkitanimationend",
    "onwebkitanimationiteration",
    "onwebkitanimationstart",
    "onwebkittransitionend",
    "onwheel",
];

/// `<html:var>` forces the HTML element of that name, so HEML keywords that
/// shadow real tags (`<var>`, `<data>`) stay reachable.
pub const HTML_ESCAPE_PREFIX: &str = "html:";

#[allow(dead_code)]
pub fn is_heml_tag(tag: &str) -> bool {
    HEML_TAGS.contains(&tag.to_ascii_lowercase().as_str())
}

pub fn is_void_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
    )
}

pub fn is_raw_text_element(tag: &str) -> bool {
    matches!(tag.to_ascii_lowercase().as_str(), "script" | "style")
}

pub fn is_html_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "html"
            | "head"
            | "title"
            | "base"
            | "link"
            | "meta"
            | "style"
            | "body"
            | "article"
            | "section"
            | "nav"
            | "aside"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "footer"
            | "address"
            | "p"
            | "hr"
            | "pre"
            | "blockquote"
            | "ol"
            | "ul"
            | "menu"
            | "li"
            | "dl"
            | "dt"
            | "dd"
            | "figure"
            | "figcaption"
            | "main"
            | "div"
            | "a"
            | "em"
            | "strong"
            | "small"
            | "s"
            | "cite"
            | "q"
            | "dfn"
            | "abbr"
            | "ruby"
            | "rt"
            | "rp"
            | "time"
            | "code"
            | "samp"
            | "kbd"
            | "sub"
            | "sup"
            | "i"
            | "b"
            | "u"
            | "mark"
            | "bdi"
            | "bdo"
            | "span"
            | "br"
            | "wbr"
            | "ins"
            | "del"
            | "picture"
            | "source"
            | "img"
            | "iframe"
            | "embed"
            | "object"
            | "param"
            | "video"
            | "audio"
            | "track"
            | "map"
            | "area"
            | "math"
            | "svg"
            | "canvas"
            | "noscript"
            | "script"
            | "table"
            | "caption"
            | "colgroup"
            | "col"
            | "tbody"
            | "thead"
            | "tfoot"
            | "tr"
            | "td"
            | "th"
            | "form"
            | "label"
            | "input"
            | "button"
            | "select"
            | "datalist"
            | "optgroup"
            | "option"
            | "textarea"
            | "output"
            | "progress"
            | "meter"
            | "fieldset"
            | "legend"
            | "details"
            | "summary"
            | "dialog"
            | "template"
            | "slot"
    )
}

pub fn is_svg_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "svg"
            | "animate"
            | "animatemotion"
            | "animatetransform"
            | "circle"
            | "clippath"
            | "defs"
            | "desc"
            | "discard"
            | "ellipse"
            | "feblend"
            | "fecolormatrix"
            | "fecomponenttransfer"
            | "fecomposite"
            | "feconvolvematrix"
            | "fediffuselighting"
            | "fedisplacementmap"
            | "fedistantlight"
            | "fedropshadow"
            | "feflood"
            | "fefunca"
            | "fefuncb"
            | "fefuncg"
            | "fefuncr"
            | "fegaussianblur"
            | "feimage"
            | "femerge"
            | "femergenode"
            | "femorphology"
            | "feoffset"
            | "fepointlight"
            | "fespecularlighting"
            | "fespotlight"
            | "fetile"
            | "feturbulence"
            | "filter"
            | "foreignobject"
            | "g"
            | "image"
            | "line"
            | "lineargradient"
            | "marker"
            | "mask"
            | "metadata"
            | "mpath"
            | "path"
            | "pattern"
            | "polygon"
            | "polyline"
            | "radialgradient"
            | "rect"
            | "set"
            | "stop"
            | "switch"
            | "symbol"
            | "text"
            | "textpath"
            | "tspan"
            | "use"
            | "view"
    )
}

pub fn is_mathml_element(tag: &str) -> bool {
    matches!(
        tag.to_ascii_lowercase().as_str(),
        "math"
            | "maction"
            | "merror"
            | "mfrac"
            | "mi"
            | "mmultiscripts"
            | "mn"
            | "mo"
            | "mover"
            | "mpadded"
            | "mphantom"
            | "mprescripts"
            | "mroot"
            | "mrow"
            | "ms"
            | "mspace"
            | "msqrt"
            | "mstyle"
            | "msub"
            | "msubsup"
            | "msup"
            | "mtable"
            | "mtd"
            | "mtext"
            | "mtr"
            | "munder"
            | "munderover"
            | "semantics"
            | "annotation"
            | "annotation-xml"
    )
}
