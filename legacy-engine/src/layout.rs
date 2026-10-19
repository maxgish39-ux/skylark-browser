//! A small, deterministic block/inline layout prototype.
//! This is not a full CSS layout implementation: no flexbox, grid, floats, or positioning.

use crate::css::ComputedStyle;
use crate::html::Node;
use crate::images::DecodedImage;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutKind {
    Document,
    Block,
    InlineText,
    InlineElement,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutBox {
    pub node_name: String,
    pub kind: LayoutKind,
    pub rect: Rect,
    pub text: Option<String>,
    pub color: u32,
    pub background: Option<u32>,
    pub font_size: i32,
    pub link_target: Option<String>,
    /// Page-space rectangles for each visible wrapped fragment of an anchor.
    pub link_fragments: Vec<Rect>,
    /// Decoded image pixels for an `img` element, if its resource loaded safely.
    pub image: Option<Arc<DecodedImage>>,
    pub children: Vec<LayoutBox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Edges {
    top: i32,
    right: i32,
    bottom: i32,
    left: i32,
}

const BLOCK_TAGS: &[&str] = &[
    "html", "body", "div", "p", "section", "article", "header", "footer", "main", "nav",
    "aside", "h1", "h2", "h3", "h4", "h5", "h6", "ul", "ol", "li", "blockquote", "pre",
    "form", "fieldset", "table", "tr", "hr",
];
const HIDDEN_TAGS: &[&str] = &["head", "style", "script", "meta", "link", "title", "base"];

fn is_block_tag(name: &str) -> bool {
    BLOCK_TAGS.iter().any(|tag| name.eq_ignore_ascii_case(tag))
}
fn is_void(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(),
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" |
        "link" | "meta" | "param" | "source" | "track" | "wbr")
}
fn is_hidden_tag(name: &str) -> bool {
    HIDDEN_TAGS.iter().any(|tag| name.eq_ignore_ascii_case(tag))
}

fn parse_length(value: &str, reference: i32, default: i32) -> i32 {
    let value = value.trim();
    if value == "auto" { return default; }
    if let Some(percent) = value.strip_suffix('%') {
        return percent.trim().parse::<f32>().ok()
            .map(|n| (reference as f32 * n / 100.0).round() as i32)
            .unwrap_or(default).max(0);
    }
    let numeric = value.strip_suffix("px").unwrap_or(value).trim();
    numeric.parse::<f32>().ok().map(|n| n.round() as i32).unwrap_or(default).max(0)
}

fn parse_edges(value: Option<&str>, reference: i32) -> Edges {
    let Some(value) = value else { return Edges::default(); };
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let parse = |s: &str| parse_length(s, reference, 0);
    match parts.as_slice() {
        [a] => Edges { top: parse(a), right: parse(a), bottom: parse(a), left: parse(a) },
        [vertical, horizontal] => Edges {
            top: parse(vertical), bottom: parse(vertical), right: parse(horizontal), left: parse(horizontal),
        },
        [top, horizontal, bottom] => Edges {
            top: parse(top), right: parse(horizontal), bottom: parse(bottom), left: parse(horizontal),
        },
        [top, right, bottom, left, ..] => Edges {
            top: parse(top), right: parse(right), bottom: parse(bottom), left: parse(left),
        },
        _ => Edges::default(),
    }
}

fn default_font_size(name: &str) -> i32 {
    match name {
        "h1" => 32,
        "h2" => 24,
        "h3" => 19,
        "h4" => 16,
        "h5" => 13,
        "h6" => 11,
        _ => 16,
    }
}

fn line_height(font_size: i32) -> i32 {
    ((font_size * 6 + 4) / 5).max(10)
}

fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 { return vec![String::new()]; }
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            if word.chars().count() > max_chars {
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max_chars) {
                    lines.push(chunk.iter().collect());
                }
            } else {
                line.push_str(word);
            }
        } else if line.chars().count() + 1 + word.chars().count() <= max_chars {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut line));
            if word.chars().count() > max_chars {
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(max_chars) {
                    lines.push(chunk.iter().collect());
                }
            } else {
                line.push_str(word);
            }
        }
    }
    if !line.is_empty() { lines.push(line); }
    lines
}

fn extract_text(node: &Node, out: &mut String) {
    if node.name == "#text" {
        if let Some(text) = &node.text {
            if out.chars().last().map(|c| !c.is_whitespace()).unwrap_or(false) { out.push(' '); }
            out.push_str(text);
        }
        return;
    }
    if node.name == "br" {
        out.push('\n');
        return;
    }
    for child in &node.children { extract_text(child, out); }
}

fn find_style<'a>(styles: &'a [(&'a str, &'a ComputedStyle)], path: &str) -> Option<&'a ComputedStyle> {
    styles.iter().find(|(candidate, _)| *candidate == path).map(|(_, style)| *style)
}

fn length_for(style: Option<&ComputedStyle>, property: &str, reference: i32, default: i32) -> i32 {
    style.and_then(|s| s.get(property)).map(|v| parse_length(v, reference, default)).unwrap_or(default)
}

fn edges_for(style: Option<&ComputedStyle>, property: &str, reference: i32) -> Edges {
    style.and_then(|s| s.get(property)).map(|v| parse_edges(Some(v), reference)).unwrap_or_default()
}

fn is_hidden(node: &Node, style: Option<&ComputedStyle>) -> bool {
    is_hidden_tag(&node.name) || style.and_then(|s| s.get("display")) == Some("none")
}

/// Lay out a document tree at a fixed viewport width. Style paths must match the paths
/// produced by `css::compute_document_styles`.
pub fn layout_document(root: &Node, styles: &std::collections::BTreeMap<String, ComputedStyle>, viewport: Viewport) -> LayoutBox {
    layout_document_with_images(root, styles, viewport, &HashMap::new())
}

/// Lay out a document with decoded images indexed by their resolved absolute URL.
pub fn layout_document_with_images(root: &Node, styles: &std::collections::BTreeMap<String, ComputedStyle>, viewport: Viewport, images: &HashMap<String, Arc<DecodedImage>>) -> LayoutBox {
    let styles_vec = styles.iter().map(|(p, s)| (p.as_str(), s)).collect::<Vec<_>>();
    let width = viewport.width.max(1);
    let mut root_box = LayoutBox {
        node_name: "#document".to_owned(), kind: LayoutKind::Document,
        rect: Rect { x: 0, y: 0, width, height: 0 }, text: None, color: 0x001F2937, background: Some(0x00FFFFFF), font_size: 16, link_target: None, link_fragments: Vec::new(), image: None, children: Vec::new(),
    };
    let mut y = 0;
    let mut indices = std::collections::HashMap::<String, usize>::new();
    for child in &root.children {
        if child.name.starts_with('#') {
            if child.name == "#text" && child.text.as_deref().unwrap_or("").trim().is_empty() { continue; }
            continue;
        }
        let index = indices.entry(child.name.clone()).or_insert(0);
        let path = format!("/{}[{}]", child.name, *index);
        *index += 1;
        if let Some(boxed) = layout_node(child, &path, &styles_vec, images, 0, y, width, viewport.height, 0x001F2937, 16) {
            y = boxed.rect.y + boxed.rect.height;
            root_box.children.push(boxed);
        }
    }
    root_box.rect.height = y.max(0);
    root_box
}

fn layout_node(
    node: &Node,
    path: &str,
    styles: &[(&str, &ComputedStyle)],
    images: &HashMap<String, Arc<DecodedImage>>,
    x: i32,
    y: i32,
    available_width: i32,
    viewport_height: i32,
    inherited_color: u32,
    inherited_font_size: i32,
) -> Option<LayoutBox> {
    let style = find_style(styles, path);
    if is_hidden(node, style) { return None; }

    let display = style.and_then(|s| s.get("display"));
    let own_color = style.and_then(|s| s.get("color")).and_then(crate::paint::parse_css_color).unwrap_or(inherited_color);
    let own_font_size = style.and_then(|s| s.get("font-size"))
        .map(|v| parse_length(v, inherited_font_size, inherited_font_size))
        .unwrap_or_else(|| match node.name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => default_font_size(&node.name),
            _ => inherited_font_size,
        })
        .clamp(8, 96);
    let block = display.map(|d| d == "block").unwrap_or_else(|| is_block_tag(&node.name));
    let margin = edges_for(style, "margin", available_width);
    let padding = edges_for(style, "padding", available_width);
    let left = x + margin.left;
    let top = y + margin.top;
    let image_src = node.attributes.get("data-ib-resolved-src").or_else(|| node.attributes.get("src"));
    let decoded_image = if node.name.eq_ignore_ascii_case("img") {
        image_src.and_then(|src| images.get(src)).cloned()
    } else { None };
    let intrinsic_width = decoded_image.as_ref().map(|img| img.width as i32).unwrap_or(24);
    let attr_width = node.attributes.get("width").and_then(|v| v.parse::<i32>().ok()).filter(|v| *v > 0);
    let specified_width = style.and_then(|s| s.get("width"))
        .map(|v| parse_length(v, available_width, if node.name.eq_ignore_ascii_case("img") { intrinsic_width } else { available_width }))
        .or(attr_width)
        .unwrap_or_else(|| if node.name.eq_ignore_ascii_case("img") { intrinsic_width } else { available_width });
    let content_width = specified_width
        .min(available_width - margin.left - margin.right - padding.left - padding.right)
        .max(1);
    let box_x = left;
    let content_x = box_x + padding.left;
    let content_y = top + padding.top;

    let kind = if block { LayoutKind::Block } else { LayoutKind::InlineElement };
    let mut children = Vec::new();
    let char_width = ((own_font_size * 3 + 2) / 5).max(4);
    let lh = line_height(own_font_size);
    let mut cursor_x = content_x;
    let mut cursor_y = content_y;
    let right_edge = content_x.saturating_add(content_width);
    let mut max_y = content_y;
    let mut child_indices = std::collections::HashMap::<String, usize>::new();

    fn emit_inline_text(
        text: &str, target: Option<&str>, color: u32, font_size: i32,
        cursor_x: &mut i32, cursor_y: &mut i32, max_y: &mut i32,
        content_x: i32, right_edge: i32, char_width: i32, lh: i32,
        children: &mut Vec<LayoutBox>,
    ) {
        // Collapse ordinary HTML whitespace between words, while preserving an
        // explicit source newline as a line break. This is not full CSS white-space.
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut first_word = true;
        for raw in normalized.split_whitespace() {
            if normalized.contains('\n') && !first_word {
                // Newlines are meaningful in preformatted content only; ordinary
                // HTML text collapses them to spaces. Keep normal flow predictable.
            }
            first_word = false;
            let word_chars = raw.chars().count() as i32;
            let word_width = word_chars.saturating_mul(char_width).max(char_width);
            let space_width = char_width;
            if *cursor_x > content_x && cursor_x.saturating_add(space_width).saturating_add(word_width) > right_edge {
                *cursor_x = content_x;
                *cursor_y = cursor_y.saturating_add(lh);
            }
            if *cursor_x > content_x {
                *cursor_x = cursor_x.saturating_add(char_width);
            }
            let visible_width = word_width.min((right_edge - *cursor_x).max(char_width));
            let rect = Rect { x: *cursor_x, y: *cursor_y, width: visible_width, height: lh };
            let mut fragments = Vec::new();
            if target.is_some() { fragments.push(rect); }
            children.push(LayoutBox {
                node_name: if target.is_some() { "a".to_owned() } else { "#text-run".to_owned() },
                kind: if target.is_some() { LayoutKind::InlineElement } else { LayoutKind::InlineText },
                rect, text: Some(raw.to_owned()), color, background: None, font_size,
                link_target: target.map(str::to_owned), link_fragments: fragments, image: None, children: Vec::new(),
            });
            *cursor_x = cursor_x.saturating_add(visible_width);
            *max_y = (*max_y).max(cursor_y.saturating_add(lh));
        }
    }

    for child in &node.children {
        if child.name == "#text" {
            if let Some(text) = &child.text {
                emit_inline_text(text, None, own_color, own_font_size, &mut cursor_x, &mut cursor_y,
                    &mut max_y, content_x, right_edge, char_width, lh, &mut children);
            }
            continue;
        }
        if child.name.starts_with('#') { continue; }
        let index = child_indices.entry(child.name.clone()).or_insert(0);
        let child_path = format!("{}/{}[{}]", path, child.name, *index);
        *index += 1;
        if child.name.eq_ignore_ascii_case("br") {
            cursor_x = content_x;
            cursor_y = cursor_y.saturating_add(lh);
            max_y = max_y.max(cursor_y);
        } else if is_block_tag(&child.name) || find_style(styles, &child_path).and_then(|s| s.get("display")) == Some("block") {
            if cursor_x > content_x { cursor_x = content_x; cursor_y = cursor_y.saturating_add(lh); }
            if let Some(child_box) = layout_node(child, &child_path, styles, images, content_x, cursor_y,
                content_width, viewport_height, own_color, own_font_size) {
                cursor_y = child_box.rect.y.saturating_add(child_box.rect.height);
                max_y = max_y.max(cursor_y);
                children.push(child_box);
            }
            cursor_x = content_x;
        } else if child.name.eq_ignore_ascii_case("img") {
            let remaining = (right_edge - cursor_x).max(1);
            if let Some(image_box) = layout_node(child, &child_path, styles, images, cursor_x, cursor_y,
                remaining, viewport_height, own_color, own_font_size) {
                if cursor_x > content_x && cursor_x.saturating_add(image_box.rect.width) > right_edge {
                    cursor_x = content_x;
                    cursor_y = cursor_y.saturating_add(lh);
                }
                let mut image_box = image_box;
                image_box.rect.x = cursor_x;
                image_box.rect.y = cursor_y;
                cursor_x = cursor_x.saturating_add(image_box.rect.width).saturating_add(char_width);
                max_y = max_y.max(cursor_y.saturating_add(image_box.rect.height));
                children.push(image_box);
            }
        } else if child.name.eq_ignore_ascii_case("a") {
            let label = child.text_content();
            let link_color = find_style(styles, &child_path).and_then(|s| s.get("color"))
                .and_then(crate::paint::parse_css_color).unwrap_or(0x001D4ED8);
            let target = child.attributes.get("href").map(String::as_str);
            emit_inline_text(&label, target, link_color, own_font_size, &mut cursor_x, &mut cursor_y,
                &mut max_y, content_x, right_edge, char_width, lh, &mut children);
        } else {
            let mut flattened = String::new();
            extract_text(child, &mut flattened);
            emit_inline_text(&flattened, None, own_color, own_font_size, &mut cursor_x, &mut cursor_y,
                &mut max_y, content_x, right_edge, char_width, lh, &mut children);
        }
    }
    let mut content_height = max_y.saturating_sub(content_y);
    if node.name == "img" {
        let intrinsic_height = decoded_image.as_ref().map(|img| img.height as i32).unwrap_or(24);
        let attr_height = node.attributes.get("height").and_then(|v| v.parse::<i32>().ok()).filter(|v| *v > 0);
        content_height = style.and_then(|s| s.get("height"))
            .map(|v| parse_length(v, viewport_height, intrinsic_height))
            .or(attr_height).unwrap_or(intrinsic_height).clamp(1, 4096);
    }
    if is_void(&node.name) && node.name != "img" && node.name != "br" && node.name != "hr" {
        content_height = 0;
    }
    if node.name == "br" { content_height = line_height(own_font_size); }
    if node.name == "hr" { content_height = 1; }

    let explicit_height = style.and_then(|s| s.get("height"))
        .map(|v| parse_length(v, viewport_height, content_height));
    let height = explicit_height.unwrap_or(content_height)
        + padding.top + padding.bottom + margin.bottom;
    let rect = Rect {
        x: box_x,
        y: top,
        width: content_width + padding.left + padding.right,
        height: height.max(0),
    };
    Some(LayoutBox {
        node_name: node.name.clone(), kind, rect, text: None,
        color: own_color,
        background: style.and_then(|s| s.get("background-color")).and_then(crate::paint::parse_css_color),
        font_size: own_font_size,
        link_target: if node.name.eq_ignore_ascii_case("a") { node.attributes.get("href").cloned() } else { None },
        link_fragments: Vec::new(), image: decoded_image, children,
    })
}

fn layout_text(text: &str, x: i32, y: i32, width: i32, font_size: i32) -> LayoutBox {
    // Approximate a character as 0.6em wide. This is only for the layout prototype.
    let char_width = ((font_size * 3 + 2) / 5).max(4);
    let max_chars = (width / char_width).max(1) as usize;
    let lines = wrap_text(text, max_chars);
    let height = lines.len() as i32 * line_height(font_size);
    LayoutBox {
        node_name: "#text-run".to_owned(),
        kind: LayoutKind::InlineText,
        rect: Rect { x, y, width, height },
        text: Some(lines.join("\n")), color: 0x001F2937, background: None, font_size, link_target: None, link_fragments: Vec::new(), image: None,
        children: Vec::new(),
    }
}

impl LayoutBox {
    /// Return the first link target whose laid-out box contains the page-space point.
    pub fn link_at(&self, x: i32, y: i32) -> Option<&str> {
        for child in self.children.iter().rev() {
            if let Some(target) = child.link_at(x, y) { return Some(target); }
        }
        if let Some(target) = self.link_target.as_deref() {
            let rects = if self.link_fragments.is_empty() { std::slice::from_ref(&self.rect) } else { &self.link_fragments };
            if rects.iter().any(|r| x >= r.x && y >= r.y && x < r.x.saturating_add(r.width)
                && y < r.y.saturating_add(r.height)) { return Some(target); }
        }
        None
    }

    pub fn format_tree(&self) -> String {
        let mut output = String::new();
        self.write_tree(&mut output, 0);
        output
    }
    fn write_tree(&self, out: &mut String, depth: usize) {
        out.push_str(&"  ".repeat(depth));
        out.push_str(&format!("{} {:?} x={} y={} w={} h={}",
            self.node_name, self.kind, self.rect.x, self.rect.y, self.rect.width, self.rect.height));
        if let Some(text) = &self.text {
            out.push_str(" text=\"");
            out.push_str(&text.replace('\n', " / "));
            out.push('"');
        }
        out.push('\n');
        for child in &self.children { child.write_tree(out, depth + 1); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{css, html};

    fn make_layout(source: &str, css_source: &str, width: i32) -> LayoutBox {
        let dom = html::parse(source);
        let rules = css::parse_stylesheet(css_source);
        let styles = css::compute_document_styles(&dom, &rules);
        layout_document(&dom, &styles, Viewport { width, height: 600 })
    }

    #[test]
    fn anchor_target_survives_layout_for_hit_testing() {
        let layout = make_layout("<p><a href=\"/next\">Next page</a></p>", "", 200);
        let anchor = &layout.children[0].children[0];
        assert_eq!(anchor.link_target.as_deref(), Some("/next"));
        assert_eq!(layout.link_at(anchor.rect.x, anchor.rect.y), Some("/next"));
    }

    #[test]
    fn image_uses_intrinsic_dimensions_when_loaded() {
        let dom = html::parse("<p>before <img src='https://example.com/a.png'> after</p>");
        let rules = css::parse_stylesheet("");
        let styles = css::compute_document_styles(&dom, &rules);
        let mut images = HashMap::new();
        images.insert("https://example.com/a.png".to_owned(), Arc::new(DecodedImage { width: 12, height: 7, pixels: vec![0; 84] }));
        let layout = layout_document_with_images(&dom, &styles, Viewport { width: 200, height: 100 }, &images);
        fn find_img(node: &LayoutBox) -> Option<&LayoutBox> {
            if node.node_name == "img" { return Some(node); }
            node.children.iter().find_map(find_img)
        }
        let img = find_img(&layout).unwrap();
        assert_eq!((img.rect.width, img.rect.height), (12, 7));
        assert!(img.image.is_some());
    }

    #[test]
    fn block_children_stack_vertically() {
        let layout = make_layout("<div><p>first</p><p>second</p></div>", "p { margin: 4px 0; }", 240);
        let div = &layout.children[0];
        assert_eq!(div.children.len(), 2);
        assert!(div.children[1].rect.y >= div.children[0].rect.y + div.children[0].rect.height);
    }

    #[test]
    fn text_wraps_at_viewport_width() {
        let layout = make_layout("<p>one two three four five six seven eight nine ten</p>", "", 70);
        let p = &layout.children[0];
        let text = p.children.first().unwrap();
        assert!(text.rect.height > 20);
        assert!(text.text.as_ref().unwrap().contains('\n'));
    }

    fn collect_links<'a>(node: &'a LayoutBox, out: &mut Vec<&'a LayoutBox>) {
        if node.link_target.is_some() { out.push(node); }
        for child in &node.children { collect_links(child, out); }
    }

    #[test]
    fn inline_text_and_link_share_a_line() {
        let layout = make_layout("<p>before <a href='/next'>linked words</a> after</p>", "", 500);
        fn find_p(node: &LayoutBox) -> Option<&LayoutBox> {
            if node.node_name == "p" { return Some(node); }
            node.children.iter().find_map(find_p)
        }
        let p = find_p(&layout).unwrap();
        let linked = p.children.iter().filter(|b| b.link_target.is_some()).collect::<Vec<_>>();
        assert_eq!(linked.len(), 2);
        assert_eq!(linked[0].rect.y, linked[1].rect.y);
        assert!(p.children.iter().any(|b| b.text.as_deref() == Some("before")));
        assert!(p.children.iter().any(|b| b.text.as_deref() == Some("after")));
    }

    #[test]
    fn wrapped_link_fragments_are_hit_tested_individually() {
        let layout = make_layout("<p><a href='/next'>one two three four five six</a></p>", "", 80);
        let mut links = Vec::new();
        collect_links(&layout, &mut links);
        assert!(links.len() >= 2);
        let first = links[0];
        assert_eq!(layout.link_at(first.rect.x, first.rect.y), Some("/next"));
        assert_eq!(layout.link_at(0, first.rect.y), None);
    }

    #[test]
    fn display_none_is_skipped() {
        let layout = make_layout("<div><p>visible</p><p class='hidden'>invisible</p></div>", ".hidden { display: none; }", 200);
        assert_eq!(layout.children[0].children.len(), 1);
    }


    #[test]
    fn foreground_color_and_font_size_are_inherited_by_text_runs() {
        let layout = make_layout("<div><p>inherited</p></div>", "div { color: #123456; font-size: 24px; }", 300);
        let text = layout.children[0].children[0].children.first().unwrap();
        assert_eq!(text.color, 0x00123456);
        assert_eq!(text.font_size, 24);
    }

    #[test]
    fn padding_and_width_affect_geometry() {
        let layout = make_layout("<div class='box'>inside words</div>", ".box { width: 100px; padding: 5px; }", 300);
        let item = &layout.children[0];
        assert_eq!(item.rect.width, 110);
        assert!(item.rect.height >= 10);
    }

    #[test]
    fn child_inherits_computed_font_size_and_color() {
        let layout = make_layout("<div style='color: #123456; font-size: 24px'><p>inherited</p></div>", "", 300);
        let div = &layout.children[0];
        let p = div.children.iter().find(|b| b.node_name == "p").unwrap();
        let text = p.children.first().unwrap();
        assert_eq!(text.color, 0x00123456);
        assert_eq!(text.font_size, 24);
    }

    #[test]
    fn inline_words_are_clipped_to_available_line_width() {
        let layout = make_layout("<p>alpha beta gamma delta</p>", "p { padding: 2px; width: 50px; }", 120);
        let p = &layout.children[0];
        assert!(p.children.iter().all(|run| run.rect.x + run.rect.width <= p.rect.x + p.rect.width));
    }

}
