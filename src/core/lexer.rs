use std::path::Path;

use crate::core::{
    error::{CompileError, Result},
    types::{is_raw_text_element, Token, TokenKind},
};

fn parse_attributes<'a>(mut input: &'a str) -> Vec<(&'a str, Option<&'a str>)> {
    let mut attributes = Vec::new();

    loop {
        input = input.trim_start();
        if input.is_empty() {
            break;
        }

        let key_end = input
            .find(|c: char| c == '=' || c.is_whitespace())
            .unwrap_or(input.len());
        if key_end == 0 {
            input = &input[1..];
            continue;
        }

        let key = &input[..key_end];
        input = input[key_end..].trim_start();

        if let Some(rest) = input.strip_prefix('=') {
            input = rest.trim_start();
            let quote = input.chars().next();
            match quote {
                Some(q @ ('"' | '\'')) => {
                    input = &input[1..];
                    match input.find(q) {
                        Some(end) => {
                            attributes.push((key, Some(&input[..end])));
                            input = &input[end + 1..];
                        }
                        None => {
                            attributes.push((key, Some(input)));
                            break;
                        }
                    }
                }
                _ => {
                    let end = input
                        .find(|c: char| c.is_whitespace())
                        .unwrap_or(input.len());
                    attributes.push((key, Some(&input[..end])));
                    input = &input[end..];
                }
            }
        } else {
            attributes.push((key, None));
        }
    }

    attributes
}

fn find_tag_end(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

fn name_end(s: &str) -> usize {
    s.find(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .unwrap_or(s.len())
}

fn find_closing(haystack: &str, tag: &str) -> Option<usize> {
    let lower = haystack.to_ascii_lowercase();
    let needle = format!("</{}", tag.to_ascii_lowercase());
    lower.find(&needle)
}

pub fn tokenize<'a>(file: &Path, source: &'a str) -> Result<Vec<Token<'a>>> {
    let mut tokens: Vec<Token<'a>> = Vec::new();
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut cursor = 0usize;

    while cursor < len {
        if bytes[cursor] == b'<' {
            let rest = &source[cursor..];

            // ---- comment ---------------------------------------------------
            if rest.starts_with("<!--") {
                let body_start = cursor + 4;
                let end = source[body_start..].find("-->").ok_or_else(|| {
                    CompileError::at(file, source, cursor, "unterminated comment")
                })?;
                tokens.push(Token::new(TokenKind::Comment(
                    &source[body_start..body_start + end],
                ), cursor));
                cursor = body_start + end + 3;
                continue;
            }

            // ---- doctype / processing instruction ---------------------------
            if rest.starts_with("<!") {
                let end = find_tag_end(&source[cursor..]).ok_or_else(|| {
                    CompileError::at(file, source, cursor, "unterminated doctype declaration")
                })?;
                let inner = source[cursor + 2..cursor + end].trim();
                let value = inner
                    .split_once(char::is_whitespace)
                    .map(|(_, v)| v.trim())
                    .unwrap_or("");
                tokens.push(Token::new(TokenKind::Doctype(value), cursor));
                cursor += end + 1;
                continue;
            }

            // ---- closing tag ------------------------------------------------
            if rest.starts_with("</") {
                let end = find_tag_end(&source[cursor..]).ok_or_else(|| {
                    CompileError::at(file, source, cursor, "unterminated closing tag")
                })?;
                let name = source[cursor + 2..cursor + end].trim();
                tokens.push(Token::new(TokenKind::TagClose(name), cursor));
                cursor += end + 1;
                continue;
            }

            // ---- opening tag ------------------------------------------------
            let after = &source[cursor + 1..];
            let looks_like_tag = after
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false);

            if looks_like_tag {
                if let Some(end) = find_tag_end(&source[cursor..]) {
                    let inner = &source[cursor + 1..cursor + end];
                    let self_closing = inner.trim_end().ends_with('/');
                    let inner = if self_closing {
                        let t = inner.trim_end();
                        &t[..t.len() - 1]
                    } else {
                        inner
                    };

                    let n = name_end(inner);
                    let tag_name = &inner[..n];
                    let attrs = parse_attributes(&inner[n..]);

                    tokens.push(Token::new(TokenKind::TagOpen {
                        name: tag_name,
                        attrs,
                        self_closing,
                    }, cursor));
                    cursor += end + 1;

                    // Raw-text elements swallow everything until their close tag,
                    // so `a < b` and `=>` inside a <script> can't be mistaken
                    // for markup.
                    if !self_closing && is_raw_text_element(tag_name) {
                        let body = &source[cursor..];
                        let rel = find_closing(body, tag_name).ok_or_else(|| {
                            CompileError::at(
                                file,
                                source,
                                cursor,
                                format!("unterminated <{}> element", tag_name),
                            )
                        })?;
                        tokens.push(Token::new(TokenKind::RawText(&body[..rel]), cursor));
                        cursor += rel;
                        let close_end = find_tag_end(&source[cursor..]).ok_or_else(|| {
                            CompileError::at(file, source, cursor, "unterminated closing tag")
                        })?;
                        tokens.push(Token::new(TokenKind::TagClose(tag_name), cursor));
                        cursor += close_end + 1;
                    }
                    continue;
                }
            }
        }

        // ---- text ---------------------------------------------------------
        let text_start = cursor;
        cursor += 1; // stray '<' that failed every branch above is literal text
        while cursor < len && bytes[cursor] != b'<' {
            cursor += 1;
        }
        let text = &source[text_start..cursor];
        if !text.is_empty() {
            tokens.push(Token::new(TokenKind::Text(text), cursor));
        }
    }

    Ok(tokens)
}
