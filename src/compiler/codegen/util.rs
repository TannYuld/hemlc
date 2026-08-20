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

/// Converts Attrs into text of `attrName="attrVal" attr...`
pub fn generate_html_from_attrs(attrs: &Attrs) -> String {
    let mut buffer = String::new();
    for (key, val) in attrs.iter() {
        buffer += key.as_str();
        if let Some(val) = val {
            buffer += &format!("=\"{}\" ", val.as_str());
        } else {
            buffer += " ";
        }
    }
    buffer.trim_end().to_string()
}