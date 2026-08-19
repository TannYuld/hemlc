use crate::{compiler::codegen::htmlgen, core::types::Attrs};

/// This converts `someAttr="RawText"` into `'RawText'`
/// and `someAttr="{3}"` into `3` (Pure js block)
pub fn parse_js_expr(val: &str) -> String {
    if val.starts_with('{') && val.ends_with('}') {
        val[1..val.len() - 1].to_string()
    } else {
        format!("`{}`", val)
    }
}

/// Minifies extra (white)spaces in raw text
pub fn minify_text(text: &str) -> String {
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

/// Builds something like this `<myTag ...>` or `<myTag .../>` (if it is void) with their respective attribtues.
pub fn build_open_tag(tag: &str, attrs: &Attrs, is_void: bool) -> String {
    if attrs.is_empty() {
        if is_void {
            format!("<{}/>", tag)
        } else {
            format!("<{}>", tag)
        }
    } else {
        let attrs = htmlgen::generate_html_from_attrs(attrs);
        if is_void {
            format!("<{} {}/>", tag, attrs)
        } else {
            format!("<{} {}>", tag, attrs)
        }
    }
}