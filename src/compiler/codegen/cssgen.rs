use crate::compiler::codegen::{HEML_SCOPE_ATTRIBUTE_KEY, types::{CSSGenerator, minify}};

pub fn scope_css(raw_css: &str, scope_id: &str) -> String {
    let mut scoped_css = String::new();

    for block in raw_css.split('}') {
        if block.trim().is_empty() {
            continue;
        }

        if let Some((selectors, rules)) = block.split_once('{') {
            let scoped_selectors: Vec<String> = selectors
                .split(',')
                .map(|s| {
                    format!(
                        "{}[{}=\"{}\"]",
                        s.trim(),
                        HEML_SCOPE_ATTRIBUTE_KEY,
                        scope_id
                    )
                })
                .collect();

            scoped_css += &format!("{} {{{}}}\n", scoped_selectors.join(", "), rules);
        }
    }
    scoped_css
}

impl CSSGenerator for minify::None {
    
}

impl CSSGenerator for minify::Js {
    
}

impl CSSGenerator for minify::All {
    
}