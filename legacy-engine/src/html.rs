//! A deliberately small, dependency-free HTML tokenizer and tree builder.
//! This is an experimental subset, not a WHATWG HTML5-compliant parser.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub attributes: BTreeMap<String, String>,
    pub text: Option<String>,
    pub children: Vec<Node>,
}

impl Node {
    fn document() -> Self {
        Self {
            name: "#document".to_owned(),
            attributes: BTreeMap::new(),
            text: None,
            children: Vec::new(),
        }
    }

    fn element(name: String, attributes: BTreeMap<String, String>) -> Self {
        Self { name, attributes, text: None, children: Vec::new() }
    }

    fn text(value: String) -> Self {
        Self { name: "#text".to_owned(), attributes: BTreeMap::new(), text: Some(value), children: Vec::new() }
    }

    fn comment(value: String) -> Self {
        Self { name: "#comment".to_owned(), attributes: BTreeMap::new(), text: Some(value), children: Vec::new() }
    }

    pub fn element_count(&self) -> usize {
        (if self.name.starts_with('#') { 0 } else { 1 }) +
            self.children.iter().map(Node::element_count).sum::<usize>()
    }

    pub fn text_content(&self) -> String {
        if self.name == "#text" {
            return self.text.clone().unwrap_or_default();
        }
        self.children.iter().map(Node::text_content).collect::<String>()
    }

    pub fn find_first(&self, name: &str) -> Option<&Node> {
        if self.name.eq_ignore_ascii_case(name) {
            return Some(self);
        }
        self.children.iter().find_map(|child| child.find_first(name))
    }

    /// Return a compact, indented tree summary suitable for diagnostics.
    pub fn format_tree(&self) -> String {
        let mut output = String::new();
        self.write_tree(&mut output, 0);
        output
    }

    fn write_tree(&self, output: &mut String, depth: usize) {
        output.push_str(&"  ".repeat(depth));
        output.push_str(&self.name);
        if !self.attributes.is_empty() {
            output.push_str(" [");
            for (index, (key, value)) in self.attributes.iter().enumerate() {
                if index > 0 { output.push_str(", "); }
                output.push_str(key);
                output.push_str("=\"");
                output.push_str(value);
                output.push('"');
            }
            output.push(']');
        }
        if let Some(text) = &self.text {
            let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !compact.is_empty() {
                output.push_str(": ");
                let preview: String = compact.chars().take(100).collect();
                output.push_str(&preview);
                if compact.chars().count() > 100 { output.push('…'); }
            }
        }
        output.push('\n');
        for child in &self.children {
            child.write_tree(output, depth + 1);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    StartTag { name: String, attributes: BTreeMap<String, String>, self_closing: bool },
    EndTag { name: String },
    Text(String),
    Comment(String),
    Doctype,
}

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
    "param", "source", "track", "wbr",
];

fn is_void(name: &str) -> bool {
    VOID_ELEMENTS.iter().any(|candidate| name.eq_ignore_ascii_case(candidate))
}

/// Tokenize enough HTML syntax to support a first browser document tree.
fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] != '<' {
            let start = i;
            while i < chars.len() && chars[i] != '<' { i += 1; }
            tokens.push(Token::Text(decode_entities(&chars_to_string(&chars[start..i]))));
            continue;
        }

        if starts_with_ascii_case(&chars, i, "<!--") {
            i += 4;
            let start = i;
            while i < chars.len() && !starts_with_ascii_case(&chars, i, "-->") { i += 1; }
            let value = chars_to_string(&chars[start..i]);
            if i < chars.len() { i += 3; }
            tokens.push(Token::Comment(value));
            continue;
        }

        if starts_with_ascii_case(&chars, i, "<!doctype") {
            while i < chars.len() && chars[i] != '>' { i += 1; }
            if i < chars.len() { i += 1; }
            tokens.push(Token::Doctype);
            continue;
        }

        if i + 1 < chars.len() && chars[i + 1] == '!' {
            while i < chars.len() && chars[i] != '>' { i += 1; }
            if i < chars.len() { i += 1; }
            continue;
        }

        if i + 1 < chars.len() && chars[i + 1] == '/' {
            i += 2;
            skip_whitespace(&chars, &mut i);
            let name = read_name(&chars, &mut i);
            while i < chars.len() && chars[i] != '>' { i += 1; }
            if i < chars.len() { i += 1; }
            if !name.is_empty() {
                tokens.push(Token::EndTag { name: name.to_ascii_lowercase() });
            }
            continue;
        }

        if i + 1 < chars.len() && is_name_char(chars[i + 1]) {
            i += 1;
            let name = read_name(&chars, &mut i).to_ascii_lowercase();
            let mut attributes = BTreeMap::new();
            let mut self_closing = false;

            loop {
                skip_whitespace(&chars, &mut i);
                if i >= chars.len() { break; }
                if chars[i] == '>' { i += 1; break; }
                if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '>' {
                    i += 2;
                    self_closing = true;
                    break;
                }

                let attr_name = read_attribute_name(&chars, &mut i).to_ascii_lowercase();
                if attr_name.is_empty() {
                    // Consume an unexpected character so malformed markup cannot stall.
                    i += 1;
                    continue;
                }
                skip_whitespace(&chars, &mut i);
                let mut value = String::new();
                if i < chars.len() && chars[i] == '=' {
                    i += 1;
                    skip_whitespace(&chars, &mut i);
                    if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                        let quote = chars[i];
                        i += 1;
                        let start = i;
                        while i < chars.len() && chars[i] != quote { i += 1; }
                        value = decode_entities(&chars_to_string(&chars[start..i]));
                        if i < chars.len() { i += 1; }
                    } else {
                        let start = i;
                        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '>' { i += 1; }
                        value = decode_entities(&chars_to_string(&chars[start..i]));
                    }
                }
                // HTML treats duplicate attributes as the first occurrence.
                attributes.entry(attr_name).or_insert(value);
            }

            tokens.push(Token::StartTag { name, attributes, self_closing });
            continue;
        }

        // A '<' that does not begin a recognized tag is ordinary text.
        tokens.push(Token::Text("<".to_owned()));
        i += 1;
    }

    tokens
}

fn chars_to_string(chars: &[char]) -> String {
    chars.iter().collect()
}

fn starts_with_ascii_case(chars: &[char], at: usize, needle: &str) -> bool {
    let needle_chars: Vec<char> = needle.chars().collect();
    at + needle_chars.len() <= chars.len()
        && chars[at..at + needle_chars.len()]
            .iter().zip(needle_chars.iter())
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn skip_whitespace(chars: &[char], i: &mut usize) {
    while *i < chars.len() && chars[*i].is_whitespace() { *i += 1; }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

fn read_name(chars: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < chars.len() && is_name_char(chars[*i]) { *i += 1; }
    chars_to_string(&chars[start..*i])
}

fn read_attribute_name(chars: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < chars.len()
        && !chars[*i].is_whitespace()
        && !matches!(chars[*i], '=' | '>' | '/')
    {
        *i += 1;
    }
    chars_to_string(&chars[start..*i])
}

fn decode_entities(input: &str) -> String {
    // Intentionally limited named-entity set; numeric references are supported.
    let mut output = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find('&') {
        output.push_str(&rest[..pos]);
        rest = &rest[pos..];
        if let Some(end) = rest.find(';') {
            let entity = &rest[1..end];
            let decoded = if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(decimal) = entity.strip_prefix('#') {
                decimal.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                match entity {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some('\u{00a0}'),
                    _ => None,
                }
            };
            if let Some(character) = decoded {
                output.push(character);
                rest = &rest[end + 1..];
            } else {
                output.push('&');
                rest = &rest[1..];
            }
        } else {
            output.push_str(rest);
            return output;
        }
    }
    output.push_str(rest);
    output
}

fn close_implied_elements(stack: &mut Vec<Node>, incoming: &str) {
    // Small, explicit subset of HTML's implied-end-tag behavior.
    if incoming == "li" {
        close_nearest(stack, "li");
    } else if incoming == "p" || matches!(incoming, "div" | "section" | "article" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "table" | "blockquote") {
        close_nearest(stack, "p");
    }
    if matches!(incoming, "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        for heading in ["h1", "h2", "h3", "h4", "h5", "h6"] {
            close_nearest(stack, heading);
        }
    }
}

fn close_nearest(stack: &mut Vec<Node>, name: &str) {
    if let Some(position) = stack.iter().rposition(|node| node.name.eq_ignore_ascii_case(name)) {
        if position > 0 {
            while stack.len() > position {
                let completed = stack.pop().expect("position is within stack");
                stack.last_mut().expect("root remains on stack").children.push(completed);
            }
        }
    }
}

/// Parse HTML into a basic document tree. Malformed closing tags are ignored safely.
pub fn parse(input: &str) -> Node {
    let tokens = tokenize(input);
    let mut root = Node::document();
    let mut stack: Vec<Node> = vec![Node::document()];

    for token in tokens {
        match token {
            Token::Doctype => {}
            Token::Text(text) => {
                if !text.is_empty() {
                    stack.last_mut().expect("document stack is never empty").children.push(Node::text(text));
                }
            }
            Token::Comment(text) => {
                stack.last_mut().expect("document stack is never empty").children.push(Node::comment(text));
            }
            Token::StartTag { name, attributes, self_closing } => {
                close_implied_elements(&mut stack, &name);
                let node = Node::element(name.clone(), attributes);
                if self_closing || is_void(&name) {
                    stack.last_mut().expect("document stack is never empty").children.push(node);
                } else {
                    stack.push(node);
                }
            }
            Token::EndTag { name } => {
                if let Some(position) = stack.iter().rposition(|node| node.name.eq_ignore_ascii_case(&name)) {
                    if position > 0 {
                        while stack.len() > position {
                            let completed = stack.pop().expect("position is within stack");
                            stack.last_mut().expect("root remains on stack").children.push(completed);
                        }
                    }
                }
            }
        }
    }

    while stack.len() > 1 {
        let completed = stack.pop().expect("non-root node exists");
        stack.last_mut().expect("root remains on stack").children.push(completed);
    }
    root.children = stack.pop().unwrap_or_default().children;
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_elements_and_text() {
        let document = parse("<html><body><h1>Hello</h1><p>world</p></body></html>");
        assert_eq!(document.element_count(), 4);
        assert_eq!(document.find_first("h1").unwrap().text_content(), "Hello");
        assert_eq!(document.find_first("p").unwrap().text_content(), "world");
    }

    #[test]
    fn reads_quoted_unquoted_and_boolean_attributes() {
        let document = parse(r#"<a href="https://example.com" title='Example' disabled data-x=one>go</a>"#);
        let link = document.find_first("a").unwrap();
        assert_eq!(link.attributes.get("href").unwrap(), "https://example.com");
        assert_eq!(link.attributes.get("title").unwrap(), "Example");
        assert_eq!(link.attributes.get("disabled").unwrap(), "");
        assert_eq!(link.attributes.get("data-x").unwrap(), "one");
    }

    #[test]
    fn handles_void_elements_and_comments() {
        let document = parse("<div><!-- note --><img src=x><br>after</div>");
        assert!(document.find_first("img").is_some());
        assert!(document.find_first("br").is_some());
        assert!(document.format_tree().contains("#comment: note"));
        assert_eq!(document.find_first("div").unwrap().text_content(), "after");
    }

    #[test]
    fn decodes_common_and_numeric_entities() {
        let document = parse("<p>A &amp; B &#x41; &#66; &lt;tag&gt;</p>");
        assert_eq!(document.find_first("p").unwrap().text_content(), "A & B A B <tag>");
    }

    #[test]
    fn implicitly_closes_list_items_and_paragraphs() {
        let document = parse("<ul><li>one<li>two</ul><p>first<p>second");
        let list = document.find_first("ul").unwrap();
        assert_eq!(list.children.iter().filter(|node| node.name == "li").count(), 2);
        assert_eq!(document.children.iter().filter(|node| node.name == "p").count(), 2);
    }

    #[test]
    fn malformed_markup_does_not_panic() {
        let _ = parse("<div><p title='unterminated>hello < b <img src=x");
    }
}
