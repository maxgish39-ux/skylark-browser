//! A small, dependency-free CSS tokenizer, declaration parser, selector matcher,
//! and cascade implementation. This is an experimental subset, not full CSS support.

use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    AtKeyword(String),
    Hash(String),
    String(String),
    Number(f32),
    Dimension(f32, String),
    Percentage(f32),
    Colon,
    Semicolon,
    Comma,
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    Delim(char),
    Whitespace,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c as u32 >= 0x80
}
fn is_name_char(c: char) -> bool {
    is_name_start(c) || c.is_ascii_digit() || c == '-'
}

/// Tokenize CSS punctuation, identifiers, strings, numbers, dimensions, comments and at-keywords.
pub fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() { i += 1; }
            out.push(Token::Whitespace);
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') { i += 1; }
            if i + 1 < chars.len() { i += 2; } else { i = chars.len(); }
            continue;
        }
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let mut value = String::new();
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 1;
                    value.push(chars[i]);
                    i += 1;
                } else {
                    value.push(chars[i]);
                    i += 1;
                }
            }
            if i < chars.len() { i += 1; }
            out.push(Token::String(value));
            continue;
        }
        if c == '@' && i + 1 < chars.len() && is_name_start(chars[i + 1]) {
            i += 1;
            let start = i;
            while i < chars.len() && is_name_char(chars[i]) { i += 1; }
            out.push(Token::AtKeyword(chars_to_string(&chars[start..i])));
            continue;
        }
        if c == '#' && i + 1 < chars.len() && is_name_char(chars[i + 1]) {
            i += 1;
            let start = i;
            while i < chars.len() && is_name_char(chars[i]) { i += 1; }
            out.push(Token::Hash(chars_to_string(&chars[start..i])));
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()) ||
            (c == '-' && i + 1 < chars.len() && (chars[i + 1].is_ascii_digit() ||
                (chars[i + 1] == '.' && i + 2 < chars.len() && chars[i + 2].is_ascii_digit()))) {
            let start = i;
            if chars[i] == '-' { i += 1; }
            while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() { i += 1; }
            }
            let number = chars_to_string(&chars[start..i]).parse::<f32>().unwrap_or(0.0);
            if i < chars.len() && chars[i] == '%' {
                i += 1;
                out.push(Token::Percentage(number));
            } else if i < chars.len() && is_name_start(chars[i]) {
                let unit_start = i;
                while i < chars.len() && is_name_char(chars[i]) { i += 1; }
                out.push(Token::Dimension(number, chars_to_string(&chars[unit_start..i]).to_ascii_lowercase()));
            } else {
                out.push(Token::Number(number));
            }
            continue;
        }
        if is_name_start(c) || c == '-' {
            let start = i;
            i += 1;
            while i < chars.len() && is_name_char(chars[i]) { i += 1; }
            out.push(Token::Ident(chars_to_string(&chars[start..i])));
            continue;
        }
        i += 1;
        out.push(match c {
            ':' => Token::Colon, ';' => Token::Semicolon, ',' => Token::Comma,
            '{' => Token::OpenBrace, '}' => Token::CloseBrace,
            '(' => Token::OpenParen, ')' => Token::CloseParen,
            other => Token::Delim(other),
        });
    }
    out
}

fn chars_to_string(chars: &[char]) -> String { chars.iter().collect() }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    pub property: String,
    pub value: String,
    pub important: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    /// Simple selectors from left to right; the last one must match the target.
    pub parts: Vec<SimpleSelector>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SimpleSelector {
    pub tag: Option<String>,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
    pub source_order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComputedStyle {
    pub properties: BTreeMap<String, String>,
}

impl ComputedStyle {
    pub fn get(&self, property: &str) -> Option<&str> {
        self.properties.get(&property.to_ascii_lowercase()).map(String::as_str)
    }
}

fn strip_comments(css: &str) -> String {
    let mut output = String::with_capacity(css.len());
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            output.push(c);
        }
    }
    output
}

fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut paren_depth = 0usize;
    for c in input.chars() {
        if let Some(q) = quote {
            current.push(c);
            if c == q { quote = None; }
            continue;
        }
        match c {
            '"' | '\'' => { quote = Some(c); current.push(c); }
            '(' => { paren_depth += 1; current.push(c); }
            ')' => { paren_depth = paren_depth.saturating_sub(1); current.push(c); }
            _ if c == delimiter && paren_depth == 0 => {
                out.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    out.push(current.trim().to_owned());
    out
}

/// Parse a semicolon-separated declaration block, preserving function values such as rgb(...).
pub fn parse_declarations(input: &str) -> Vec<Declaration> {
    let cleaned = strip_comments(input);
    let mut result = Vec::new();
    for raw in split_top_level(&cleaned, ';') {
        if raw.is_empty() { continue; }
        let Some(colon) = raw.find(':') else { continue; };
        let property = raw[..colon].trim().to_ascii_lowercase();
        if property.is_empty() { continue; }
        let mut value = raw[colon + 1..].trim().to_owned();
        let important = value.to_ascii_lowercase().ends_with("!important");
        if important {
            let cut = value.len() - "!important".len();
            value = value[..cut].trim_end().to_owned();
        }
        if !value.is_empty() {
            result.push(Declaration { property, value, important });
        }
    }
    result
}

fn parse_compound(input: &str) -> Option<SimpleSelector> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut simple = SimpleSelector::default();
    if i < chars.len() && (is_name_start(chars[i]) || chars[i] == '*') {
        if chars[i] == '*' {
            i += 1;
        } else {
            let start = i;
            i += 1;
            while i < chars.len() && is_name_char(chars[i]) { i += 1; }
            simple.tag = Some(chars_to_string(&chars[start..i]).to_ascii_lowercase());
        }
    }
    while i < chars.len() {
        let marker = chars[i];
        if marker != '.' && marker != '#' { return None; }
        i += 1;
        let start = i;
        while i < chars.len() && is_name_char(chars[i]) { i += 1; }
        if start == i { return None; }
        let name = chars_to_string(&chars[start..i]);
        if marker == '#' { simple.id = Some(name); } else { simple.classes.push(name); }
    }
    if simple.tag.is_none() && simple.id.is_none() && simple.classes.is_empty() {
        None
    } else {
        Some(simple)
    }
}

/// Parse comma-separated tag, class, ID, and descendant selectors.
pub fn parse_selector(input: &str) -> Option<Selector> {
    let normalized = input.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() { return None; }
    let mut parts = Vec::new();
    for part in normalized.split(' ') {
        parts.push(parse_compound(part)?);
    }
    Some(Selector { parts })
}

pub fn parse_stylesheet(input: &str) -> Vec<Rule> {
    let cleaned = strip_comments(input);
    let mut rules = Vec::new();
    let mut cursor = 0;
    let mut source_order = 0;
    while let Some(relative_open) = cleaned[cursor..].find('{') {
        let open = cursor + relative_open;
        let Some(relative_close) = cleaned[open + 1..].find('}') else { break; };
        let close = open + 1 + relative_close;
        let selector_text = cleaned[cursor..open].trim();
        let body = &cleaned[open + 1..close];
        cursor = close + 1;
        if selector_text.is_empty() || selector_text.starts_with('@') { continue; }
        let selectors = split_top_level(selector_text, ',')
            .iter().filter_map(|s| parse_selector(s)).collect::<Vec<_>>();
        let declarations = parse_declarations(body);
        if !selectors.is_empty() && !declarations.is_empty() {
            rules.push(Rule { selectors, declarations, source_order });
            source_order += 1;
        }
    }
    rules
}

fn attribute<'a>(node: &'a crate::html::Node, name: &str) -> Option<&'a str> {
    node.attributes.get(name).map(String::as_str)
}

fn simple_matches(simple: &SimpleSelector, node: &crate::html::Node) -> bool {
    if node.name.starts_with('#') { return false; }
    if let Some(tag) = &simple.tag {
        if !node.name.eq_ignore_ascii_case(tag) { return false; }
    }
    if let Some(id) = &simple.id {
        if attribute(node, "id") != Some(id.as_str()) { return false; }
    }
    if !simple.classes.is_empty() {
        let classes = attribute(node, "class").unwrap_or("");
        let class_set = classes.split_whitespace().collect::<Vec<_>>();
        if !simple.classes.iter().all(|class| class_set.contains(&class.as_str())) { return false; }
    }
    true
}

fn selector_matches(selector: &Selector, node: &crate::html::Node, ancestors: &[&crate::html::Node]) -> bool {
    if selector.parts.is_empty() { return false; }
    let last = selector.parts.len() - 1;
    if !simple_matches(&selector.parts[last], node) { return false; }
    if last == 0 { return true; }
    let mut ancestor_index = ancestors.len();
    for part in selector.parts[..last].iter().rev() {
        let mut found = false;
        while ancestor_index > 0 {
            ancestor_index -= 1;
            if simple_matches(part, ancestors[ancestor_index]) {
                found = true;
                break;
            }
        }
        if !found { return false; }
    }
    true
}

fn specificity(selector: &Selector) -> (u32, u32, u32) {
    let mut ids = 0;
    let mut classes = 0;
    let mut tags = 0;
    for part in &selector.parts {
        if part.id.is_some() { ids += 1; }
        classes += part.classes.len() as u32;
        if part.tag.is_some() { tags += 1; }
    }
    (ids, classes, tags)
}

/// Compute supported declarations for one node. `ancestors` are ordered outermost to innermost.
/// This first cascade implementation handles `!important`, specificity, and source order for
/// author rules only; inherited/default browser styles are not yet included.
pub fn compute_style(
    node: &crate::html::Node,
    ancestors: &[&crate::html::Node],
    rules: &[Rule],
    inline_style: Option<&str>,
) -> ComputedStyle {
    let mut chosen: HashMap<String, ((bool, (u32, u32, u32), usize), String)> = HashMap::new();
    for rule in rules {
        let matching = rule.selectors.iter()
            .filter(|selector| selector_matches(selector, node, ancestors))
            .map(specificity).max();
        if let Some(spec) = matching {
            for declaration in &rule.declarations {
                let key = (declaration.important, spec, rule.source_order);
                let replace = chosen.get(&declaration.property).map(|(old, _)| key > *old).unwrap_or(true);
                if replace {
                    chosen.insert(declaration.property.clone(), (key, declaration.value.clone()));
                }
            }
        }
    }
    if let Some(inline) = inline_style {
        for declaration in parse_declarations(inline) {
            let key = (declaration.important, (1_000_000, 0, 0), usize::MAX);
            let replace = chosen.get(&declaration.property).map(|(old, _)| key > *old).unwrap_or(true);
            if replace {
                chosen.insert(declaration.property, (key, declaration.value));
            }
        }
    }
    let mut properties = BTreeMap::new();
    for (name, (_, value)) in chosen {
        properties.insert(name, value);
    }
    ComputedStyle { properties }
}

/// Collect styles for the whole DOM, returning a path-keyed map (`/html/body/p[0]`).
pub fn compute_document_styles(root: &crate::html::Node, rules: &[Rule]) -> BTreeMap<String, ComputedStyle> {
    fn walk(
        node: &crate::html::Node,
        path: &str,
        ancestors: &[&crate::html::Node],
        rules: &[Rule],
        out: &mut BTreeMap<String, ComputedStyle>,
        inherited: &ComputedStyle,
    ) {
        let inline = node.attributes.get("style").map(String::as_str);
        let mut computed = if !node.name.starts_with('#') {
            compute_style(node, ancestors, rules, inline)
        } else {
            ComputedStyle::default()
        };
        // CSS properties inherited by ordinary descendants. This is intentionally a
        // small supported subset, not a complete CSS computed-value algorithm.
        const INHERITED: &[&str] = &["color", "font-family", "font-size", "font-style", "font-weight", "line-height", "text-align", "visibility", "white-space"];
        for property in INHERITED {
            if computed.get(property).is_none() {
                if let Some(value) = inherited.get(property) {
                    computed.properties.insert((*property).to_owned(), value.to_owned());
                }
            }
        }
        if !node.name.starts_with('#') {
            out.insert(path.to_owned(), computed.clone());
        }
        let mut element_indices: HashMap<&str, usize> = HashMap::new();
        for child in &node.children {
            if child.name.starts_with('#') {
                walk(child, path, ancestors, rules, out, &computed);
                continue;
            }
            let index = element_indices.entry(child.name.as_str()).or_insert(0);
            let child_path = format!("{}/{}[{}]", path, child.name, *index);
            *index += 1;
            let mut child_ancestors = ancestors.to_vec();
            child_ancestors.push(node);
            walk(child, &child_path, &child_ancestors, rules, out, &computed);
        }
    }
    let mut out = BTreeMap::new();
    walk(root, "", &[], rules, &mut out, &ComputedStyle::default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html;

    #[test]
    fn tokenizer_recognizes_css_tokens_and_comments() {
        let tokens = tokenize("/* c */ h1 { width: 12px; color: #fff }");
        assert!(tokens.iter().any(|t| matches!(t, Token::Ident(s) if s == "h1")));
        assert!(tokens.iter().any(|t| matches!(t, Token::Dimension(12.0, unit) if unit == "px")));
        assert!(tokens.iter().any(|t| matches!(t, Token::Hash(s) if s == "fff")));
        assert!(!tokens.iter().any(|t| matches!(t, Token::Ident(s) if s == "c")));
    }

    #[test]
    fn parses_declarations_and_important() {
        let decls = parse_declarations("color: red; width: 12px; background: rgb(1, 2, 3) !important;");
        assert_eq!(decls.len(), 3);
        assert_eq!(decls[1].value, "12px");
        assert!(decls[2].important);
        assert_eq!(decls[2].value, "rgb(1, 2, 3)");
    }

    #[test]
    fn parses_basic_selectors() {
        assert!(parse_selector("div.card #title").is_some());
        assert!(parse_selector("p.note.active").is_some());
        assert!(parse_selector("##broken").is_none());
    }

    #[test]
    fn matches_descendant_selector_and_computes_cascade() {
        let dom = html::parse(r#"<div class="card"><p id="intro" class="lead">Hello</p></div>"#);
        let rules = parse_stylesheet(
            "p { color: red; } .card p { color: blue; } #intro { color: green; } p.lead { font-size: 18px; }"
        );
        let styles = compute_document_styles(&dom, &rules);
        let p = styles.values().find(|style| style.get("font-size") == Some("18px")).unwrap();
        assert_eq!(p.get("color"), Some("green"));
    }

    #[test]
    fn inline_style_wins_over_normal_author_rule() {
        let dom = html::parse(r#"<p style="color: purple">Hello</p>"#);
        let rules = parse_stylesheet("p { color: red; }");
        let styles = compute_document_styles(&dom, &rules);
        assert!(styles.values().any(|style| style.get("color") == Some("purple")));
    }

    #[test]
    fn ignores_at_rules_in_basic_stylesheet_parser() {
        let rules = parse_stylesheet("@media screen { p { color: red; } } p { color: blue; }");
        assert!(rules.iter().all(|rule| rule.selectors.iter().all(|s| s.parts.last().and_then(|p| p.tag.as_deref()) != Some("@media screen"))));
    }

    #[test]
    fn inherited_properties_flow_to_nested_descendants_but_not_box_properties() {
        let dom = html::parse("<div style=\"color: #123456; font-size: 22px; margin: 9px;\"><p><a href='/x'>link</a></p></div>");
        let styles = compute_document_styles(&dom, &[]);
        let anchor = styles.iter().find(|(path, _)| path.ends_with("/a[0]")).unwrap().1;
        assert_eq!(anchor.get("color"), Some("#123456"));
        assert_eq!(anchor.get("font-size"), Some("22px"));
        assert_eq!(anchor.get("margin"), None);
    }

}
