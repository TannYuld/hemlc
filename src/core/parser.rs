use std::path::Path;

use crate::core::{
    error::{CompileError, Result},
    types::*,
};

pub fn parse<'a>(file: &Path, src: &'a str, tokens: &'a [Token<'a>]) -> Result<Document> {
    let mut p = Parser {
        file,
        src,
        toks: tokens,
        pos: 0,
    };
    let nodes = p.parse_nodes(None)?;
    Ok(Document { nodes })
}

struct Parser<'a, 'b> {
    file: &'b Path,
    src: &'a str,
    toks: &'a [Token<'a>],
    pos: usize,
}

/// Sibling-level item. `<elseif>` / `<else>` are separate tags in the source but
/// belong to the preceding `<if>`, so they are collected here and merged after
/// the whole sibling list is known.
enum Item {
    Node(Node),
    ElseIf {
        condition: String,
        body: Vec<Node>,
        start: usize,
    },
    Else {
        body: Vec<Node>,
        start: usize,
    },
}

impl<'a, 'b> Parser<'a, 'b> {
    fn err(&self, offset: usize, msg: impl Into<String>) -> CompileError {
        CompileError::at(self.file, self.src, offset, msg)
    }

    fn peek(&self) -> Option<&'a Token<'a>> {
        self.toks.get(self.pos)
    }

    fn parse_nodes(&mut self, stop: Option<&str>) -> Result<Vec<Node>> {
        let mut items: Vec<Item> = Vec::new();

        while let Some(tok) = self.peek() {
            match &tok.kind {
                TokenKind::TagClose(name) => {
                    if let Some(expected) = stop {
                        if name.eq_ignore_ascii_case(expected) {
                            self.pos += 1;
                            return self.merge(items);
                        }
                    }
                    if is_void_element(name) {
                        // Tolerate the `</br>` idiom: treat it as `<br/>`.
                        items.push(Item::Node(Node::new(
                            NodeKind::Element {
                                tag: name.to_ascii_lowercase(),
                                attrs: Attrs::empty(),
                                children: Vec::new(),
                                void: true,
                            },
                            tok.start,
                        )));
                        self.pos += 1;
                        continue;
                    }
                    return Err(self.err(
                        tok.start,
                        match stop {
                            Some(open) => format!(
                                "unexpected `</{}>`; the innermost open element is `<{}>`",
                                name, open
                            ),
                            None => format!("unexpected `</{}>`; no element is open here", name),
                        },
                    ));
                }
                TokenKind::Text(t) => {
                    items.push(Item::Node(Node::new(
                        NodeKind::Text((*t).to_string()),
                        tok.start,
                    )));
                    self.pos += 1;
                }
                TokenKind::Comment(c) => {
                    items.push(Item::Node(Node::new(
                        NodeKind::Comment((*c).to_string()),
                        tok.start,
                    )));
                    self.pos += 1;
                }
                TokenKind::Doctype(d) => {
                    items.push(Item::Node(Node::new(
                        NodeKind::Doctype((*d).to_string()),
                        tok.start,
                    )));
                    self.pos += 1;
                }
                TokenKind::RawText(_) => {
                    // Only reachable directly after a raw-text open tag, which
                    // parse_element consumes itself.
                    self.pos += 1;
                }
                TokenKind::TagOpen { .. } => {
                    let item = self.parse_element()?;
                    items.push(item);
                }
            }
        }

        if let Some(expected) = stop {
            let at = self.toks.last().map(|t| t.start).unwrap_or(self.src.len());
            return Err(self.err(at, format!("unclosed `<{}>` element", expected)));
        }

        self.merge(items)
    }

    fn parse_element(&mut self) -> Result<Item> {
        let tok = self.toks[self.pos].clone();
        let (name, attrs, self_closing) = match tok.kind {
            TokenKind::TagOpen {
                name,
                ref attrs,
                self_closing,
            } => (name, attrs, self_closing),
            _ => unreachable!(),
        };
        let start = tok.start;
        self.pos += 1;

        let lower = name.to_ascii_lowercase();
        // `<html:var>` / `<html:data>`: force the HTML element, bypassing the
        // HEML keyword table entirely.
        let html_forced = lower.starts_with(HTML_ESCAPE_PREFIX);
        let lower = match html_forced {
            true => lower[HTML_ESCAPE_PREFIX.len()..].to_string(),
            false => lower,
        };
        let owned_attrs: Attrs = attrs
            .iter()
            .map(|(k, v)| Attr {
                key: (*k).to_string(),
                value: if let Some(val) = v {
                    Some(val.to_string())
                } else {
                    None
                },
            })
            .collect();

        // Raw-text elements: <script>/<style> bodies pass through verbatim.
        if is_raw_text_element(&lower) && !self_closing {
            let content = match self.peek().map(|t| &t.kind) {
                Some(TokenKind::RawText(c)) => {
                    let c = (*c).to_string();
                    self.pos += 1;
                    c
                }
                _ => String::new(),
            };
            if matches!(self.peek().map(|t| &t.kind), Some(TokenKind::TagClose(_))) {
                self.pos += 1;
            }
            return Ok(Item::Node(Node::new(
                NodeKind::Raw {
                    tag: lower,
                    attrs: owned_attrs,
                    content,
                },
                start,
            )));
        }

        let leaf = self_closing || is_void_element(&lower);

        if html_forced {
            let children = if leaf {
                Vec::new()
            } else {
                self.parse_nodes(Some(name))?
            };
            return Ok(Item::Node(Node::new(
                NodeKind::Element {
                    tag: lower.clone(),
                    attrs: owned_attrs,
                    children,
                    void: is_void_element(&lower),
                },
                start,
            )));
        }

        // `<match>` accepts only `<arm>` children, so it gets its own loop.
        if lower == "match" && !leaf {
            let value = owned_attrs
                .attr("value")
                .map(|s| s.to_string())
                .ok_or_else(|| self.err(start, "`<match>` requires a `value` attribute"))?;
            let arms = self.parse_arms(name, start)?;
            return Ok(Item::Node(Node::new(
                NodeKind::Match { value, arms },
                start,
            )));
        }

        let children = if leaf {
            Vec::new()
        } else {
            self.parse_nodes(Some(name))?
        };

        let get = |key: &str| owned_attrs.attr(key).map(|s| s.to_string());
        let require = |key: &str| -> Result<String> {
            get(key).ok_or_else(|| {
                self.err(
                    start,
                    format!("`<{}>` requires a `{}` attribute", lower, key),
                )
            })
        };
        let has_property = |name: &str| -> bool {
            owned_attrs
                .iter()
                .any(|(key, _)| key.eq_ignore_ascii_case(name))
        };

        let kind = match lower.as_str() {
            "import" => {
                let src = match get("src") {
                    Some(s) => s,
                    None => {
                        // `<import type="component">./mybutton.html</import>`
                        let text: String = children
                            .iter()
                            .filter_map(|c| match &c.kind {
                                NodeKind::Text(t) => Some(t.trim()),
                                _ => None,
                            })
                            .collect();
                        if text.is_empty() {
                            return Err(self.err(
                                start,
                                "`<import>` needs a `src` attribute or a path as its body",
                            ));
                        }
                        text
                    }
                };
                NodeKind::Import {
                    src,
                    alias: get("as"),
                }
            }
            "properties" => NodeKind::Properties {
                properties: children,
            },
            // "property" => NodeKind::Property {
            //     name: require("name")?,
            //     value: require("value")?,
            //     body: text_childeren_from_childeren(children)?,
            // },
            "attribute" => NodeKind::Attribute {
                name: require("name")?,
                optional: has_property("optional"),
            },
            "component" => NodeKind::Component {
                childeren: children,
            },
            "var" => NodeKind::Var {
                name: require("name")?,
                value: get("value"),
            },
            "if" => NodeKind::If {
                branches: vec![Branch {
                    condition: require("condition")?,
                    body: children,
                }],
                otherwise: None,
            },
            "elseif" => {
                return Ok(Item::ElseIf {
                    condition: require("condition")?,
                    body: children,
                    start,
                });
            }
            "else" => {
                return Ok(Item::Else {
                    body: children,
                    start,
                });
            }
            "arm" => return Err(self.err(start, "`<arm>` is only valid directly inside `<match>`")),
            "for" => NodeKind::For {
                each: require("each")?,
                binding: require("as")?,
                index: get("index"),
                key: get("key"),
                body: children,
            },
            "value" => NodeKind::Value {
                name: require("name")?,
                fixed: has_property("fixed"),
            },
            "data" => NodeKind::Data {
                path: require("name")?,
                body: children,
            },
            "key" => NodeKind::Key {
                path: require("name")?,
                body: children,
            },
            "children" => NodeKind::Slot,
            _ if is_html_element(&lower) | is_mathml_element(&lower) | is_svg_element(&lower) => {
                NodeKind::Element {
                    tag: lower.clone(),
                    attrs: owned_attrs,
                    children,
                    void: is_void_element(&lower),
                }
            }
            _ => NodeKind::Unknown {
                tag: lower.clone(),
                attrs: owned_attrs,
                children,
            },
        };

        Ok(Item::Node(Node::new(kind, start)))
    }

    fn parse_arms(&mut self, match_tag: &str, match_start: usize) -> Result<Vec<Arm>> {
        let mut arms = Vec::new();

        loop {
            let Some(tok) = self.peek() else {
                return Err(self.err(match_start, "unclosed `<match>` element"));
            };
            let start = tok.start;
            match &tok.kind {
                TokenKind::Text(t) if t.trim().is_empty() => self.pos += 1,
                TokenKind::Comment(_) => self.pos += 1,
                TokenKind::TagClose(name) if name.eq_ignore_ascii_case(match_tag) => {
                    self.pos += 1;
                    break;
                }
                TokenKind::TagOpen {
                    name,
                    attrs,
                    self_closing,
                } if name.eq_ignore_ascii_case("arm") => {
                    let is_default = attrs.iter().any(|(k, _)| k.eq_ignore_ascii_case("default"));
                    let expr = attrs
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case("expr"))
                        .map(|(_, v)| {
                            let v = *v;
                            if let Some(val) = v {
                                Some(val.to_string())
                            } else {
                                None
                            }
                        });
                    if expr.is_none() && !is_default {
                        return Err(
                            self.err(start, "`<arm>` needs either `expr=\"...\"` or `default`")
                        );
                    }
                    let sc = *self_closing;
                    self.pos += 1;
                    let body = if sc {
                        Vec::new()
                    } else {
                        self.parse_nodes(Some("arm"))?
                    };
                    arms.push(Arm {
                        expr: if is_default {
                            None
                        } else {
                            if let Some(val) = expr {
                                if let Some(v) = val { Some(v) } else { None }
                            } else {
                                None
                            }
                        },
                        body,
                    });
                }
                _ => {
                    return Err(
                        self.err(start, "only `<arm>` elements may appear inside `<match>`")
                    );
                }
            }
        }

        if arms.is_empty() {
            return Err(self.err(match_start, "`<match>` needs at least one `<arm>`"));
        }
        if let Some(i) = arms.iter().position(|a| a.expr.is_none()) {
            if i != arms.len() - 1 {
                return Err(self.err(match_start, "the default `<arm>` must come last"));
            }
        }
        Ok(arms)
    }

    /// Attach `<elseif>` / `<else>` to the `<if>` that precedes them.
    fn merge(&self, items: Vec<Item>) -> Result<Vec<Node>> {
        let mut out: Vec<Node> = Vec::new();
        // Whitespace sitting between `</if>` and `<else>` is held back until we
        // know whether the chain continues.
        let mut pending_ws: Vec<Node> = Vec::new();

        for item in items {
            match item {
                Item::Node(node) => {
                    if matches!(&node.kind, NodeKind::Text(t) if t.trim().is_empty()) {
                        pending_ws.push(node);
                    } else {
                        out.append(&mut pending_ws);
                        out.push(node);
                    }
                }
                Item::ElseIf {
                    condition,
                    body,
                    start,
                } => {
                    let target = out.last_mut().ok_or_else(|| {
                        self.err(start, "`<elseif>` must follow an `<if>` or `<elseif>`")
                    })?;
                    match &mut target.kind {
                        NodeKind::If {
                            branches,
                            otherwise,
                        } if otherwise.is_none() => {
                            branches.push(Branch { condition, body });
                        }
                        _ => {
                            return Err(
                                self.err(start, "`<elseif>` must follow an `<if>` or `<elseif>`")
                            );
                        }
                    }
                    pending_ws.clear();
                }
                Item::Else { body, start } => {
                    let target = out
                        .last_mut()
                        .ok_or_else(|| self.err(start, "`<else>` must follow an `<if>`"))?;
                    match &mut target.kind {
                        NodeKind::If { otherwise, .. } if otherwise.is_none() => {
                            *otherwise = Some(body);
                        }
                        _ => return Err(self.err(start, "`<else>` must follow an `<if>`")),
                    }
                    pending_ws.clear();
                }
            }
        }
        out.append(&mut pending_ws);
        Ok(out)
    }
}
