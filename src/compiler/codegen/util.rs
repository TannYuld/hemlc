use crate::core::types::Attrs;

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
        .is_some_and(|c| c.is_ascii_whitespace())
    {
        minified.insert(0, ' ');
    }

    if text
        .chars()
        .last()
        .is_some_and(|c| c.is_ascii_whitespace())
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

/// Extract `[myObservable]` from condition of such conditionals `<if condition="{myObservable.value === 3 && myNonObservable === 5}">`
pub fn extract_observables(expr: &str) -> Vec<String> {
    let mut deps = std::collections::HashSet::new();
    let mut chars = expr.chars().peekable();
    let mut current_word = String::new();
    let mut is_property = false;

    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() || c == '_' || c == '$' || (c.is_ascii_digit() && !current_word.is_empty()) {
            current_word.push(c);
            chars.next();
        } else {
            if !current_word.is_empty() {
                let mut next_char = ' ';
                let mut lookahead = chars.clone();
                
                while let Some(&nc) = lookahead.peek() {
                    if !nc.is_ascii_whitespace() {
                        next_char = nc;
                        break;
                    }
                    lookahead.next();
                }

                if !is_property && next_char != '(' && next_char != ':' && !is_js_keyword(&current_word) {
                    deps.insert(current_word.clone());
                }
                current_word.clear();
            }

            let c = chars.next().unwrap();
            if c == '.' {
                is_property = true;
            } else if !c.is_ascii_whitespace() {
                is_property = false; 
            }
        }
    }
    
    if !current_word.is_empty() && !is_property && !is_js_keyword(&current_word) {
        deps.insert(current_word);
    }
    
    deps.into_iter().collect()
}

fn is_js_keyword(word: &str) -> bool {
    matches!(word, "true" | "false" | "null" | "undefined" | "Math" | "JSON" | "window" | "document" | "console")
}