//! Renderer-independent paint commands and a tiny software rasterizer.
//! This is a deliberate first step: solid fills, 1px borders, and 8x8 bitmap text.

use crate::layout::{LayoutBox, LayoutKind, Rect};
use crate::images::DecodedImage;
use font8x8::{UnicodeFonts, BASIC_FONTS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintCommand {
    FillRect { rect: Rect, color: u32 },
    BorderRect { rect: Rect, color: u32 },
    Text { x: i32, y: i32, text: String, color: u32 },
    Image { rect: Rect, image: std::sync::Arc<DecodedImage> },
}

const PAGE_WHITE: u32 = 0x00FFFFFF;
const INK: u32 = 0x001F2937;
const LINK_BLUE: u32 = 0x001D4ED8;
const RULE: u32 = 0x00D1D5DB;
const SOFT_PANEL: u32 = 0x00F3F4F6;

/// Parse the supported CSS color subset into minifb's 0x00RRGGBB pixel format.
pub fn parse_css_color(input: &str) -> Option<u32> {
    let value = input.trim().to_ascii_lowercase();
    let named = match value.as_str() {
        "black" => Some(0x00000000), "white" => Some(0x00FFFFFF),
        "red" => Some(0x00FF0000), "green" => Some(0x00008000),
        "blue" => Some(0x000000FF), "yellow" => Some(0x00FFFF00),
        "gray" | "grey" => Some(0x00808080), "silver" => Some(0x00C0C0C0),
        "maroon" => Some(0x00800000), "purple" => Some(0x00800080),
        "fuchsia" => Some(0x00FF00FF), "lime" => Some(0x0000FF00),
        "olive" => Some(0x00808000), "navy" => Some(0x00000080),
        "teal" => Some(0x00008080), "aqua" | "cyan" => Some(0x0000FFFF),
        "orange" => Some(0x00FFA500), "rebeccapurple" => Some(0x00663399),
        "transparent" => Some(0), _ => None,
    };
    if named.is_some() { return named; }
    if let Some(hex) = value.strip_prefix('#') {
        return match hex.len() {
            3 => {
                let mut out = 0u32;
                for ch in hex.chars() { out = (out << 8) | u32::from(u8::from_str_radix(&ch.to_string().repeat(2), 16).ok()?); }
                Some(out)
            }
            6 => u32::from_str_radix(hex, 16).ok(),
            _ => None,
        };
    }
    if let Some(body) = value.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let parts: Vec<&str> = body.split(',').map(str::trim).collect();
        if parts.len() != 3 { return None; }
        let mut rgb = [0u32; 3];
        for (i, part) in parts.iter().enumerate() {
            let n = part.parse::<u16>().ok()?;
            if n > 255 { return None; }
            rgb[i] = n as u32;
        }
        return Some((rgb[0] << 16) | (rgb[1] << 8) | rgb[2]);
    }
    None
}

pub fn build_paint_list(root: &LayoutBox) -> Vec<PaintCommand> {
    let mut commands = vec![PaintCommand::FillRect {
        rect: Rect { x: 0, y: 0, width: root.rect.width.max(1), height: root.rect.height.max(1) },
        color: PAGE_WHITE,
    }];
    visit(root, &mut commands);
    commands
}

fn visit(node: &LayoutBox, commands: &mut Vec<PaintCommand>) {
    if node.kind == LayoutKind::Hidden { return; }
    let rect = node.rect;
    if rect.width > 0 && rect.height > 0 {
        if let Some(color) = node.background {
            commands.push(PaintCommand::FillRect { rect, color });
        }
        if node.background.is_none() {
            match node.node_name.as_str() {
                "hr" => commands.push(PaintCommand::FillRect { rect, color: RULE }),
                "pre" | "code" => commands.push(PaintCommand::FillRect { rect, color: SOFT_PANEL }),
                "blockquote" => commands.push(PaintCommand::FillRect { rect: Rect { x: rect.x, y: rect.y, width: 3, height: rect.height }, color: RULE }),
                _ => {}
            }
        }
    }
    if let Some(image) = &node.image {
        commands.push(PaintCommand::Image { rect, image: image.clone() });
    }
    if let Some(text) = &node.text {
        let color = if node.link_target.is_some() || node.node_name == "a" { LINK_BLUE } else { node.color };
        commands.push(PaintCommand::Text { x: rect.x, y: rect.y, text: text.clone(), color });
    }
    for child in &node.children { visit(child, commands); }
}

pub fn rasterize(width: usize, height: usize, background: u32, commands: &[PaintCommand]) -> Vec<u32> {
    let len = width.saturating_mul(height);
    let mut buffer = vec![background; len];
    if width == 0 || height == 0 { return buffer; }
    for command in commands {
        match command {
            PaintCommand::FillRect { rect, color } => fill_rect(&mut buffer, width, height, *rect, *color),
            PaintCommand::BorderRect { rect, color } => {
                fill_rect(&mut buffer, width, height, Rect { x: rect.x, y: rect.y, width: rect.width, height: 1 }, *color);
                fill_rect(&mut buffer, width, height, Rect { x: rect.x, y: rect.y.saturating_add(rect.height).saturating_sub(1), width: rect.width, height: 1 }, *color);
                fill_rect(&mut buffer, width, height, Rect { x: rect.x, y: rect.y, width: 1, height: rect.height }, *color);
                fill_rect(&mut buffer, width, height, Rect { x: rect.x.saturating_add(rect.width).saturating_sub(1), y: rect.y, width: 1, height: rect.height }, *color);
            }
            PaintCommand::Text { x, y, text, color } => draw_text(&mut buffer, width, height, *x, *y, text, *color),
            PaintCommand::Image { rect, image } => draw_image(&mut buffer, width, height, *rect, image),
        }
    }
    buffer
}


fn draw_image(buffer: &mut [u32], canvas_width: usize, canvas_height: usize, rect: Rect, image: &DecodedImage) {
    if rect.width <= 0 || rect.height <= 0 || image.width == 0 || image.height == 0
        || image.pixels.len() != (image.width as usize).saturating_mul(image.height as usize) { return; }
    let left = rect.x.max(0) as usize;
    let top = rect.y.max(0) as usize;
    let right = rect.x.saturating_add(rect.width).max(0).min(canvas_width as i32) as usize;
    let bottom = rect.y.saturating_add(rect.height).max(0).min(canvas_height as i32) as usize;
    if left >= right || top >= bottom { return; }
    for y in top..bottom {
        let source_y = (((y as i64 - i64::from(rect.y)) * i64::from(image.height)) / i64::from(rect.height))
            .clamp(0, i64::from(image.height - 1)) as usize;
        for x in left..right {
            let source_x = (((x as i64 - i64::from(rect.x)) * i64::from(image.width)) / i64::from(rect.width))
                .clamp(0, i64::from(image.width - 1)) as usize;
            let source_index = source_y * image.width as usize + source_x;
            buffer[y * canvas_width + x] = image.pixels[source_index];
        }
    }
}

fn fill_rect(buffer: &mut [u32], width: usize, height: usize, rect: Rect, color: u32) {
    if rect.width <= 0 || rect.height <= 0 { return; }
    let left = rect.x.max(0) as usize;
    let top = rect.y.max(0) as usize;
    let right = (rect.x.saturating_add(rect.width)).max(0).min(width as i32) as usize;
    let bottom = (rect.y.saturating_add(rect.height)).max(0).min(height as i32) as usize;
    if left >= right || top >= bottom { return; }
    for y in top..bottom {
        let start = y * width + left;
        buffer[start..y * width + right].fill(color);
    }
}

fn draw_text(buffer: &mut [u32], width: usize, height: usize, x: i32, y: i32, text: &str, color: u32) {
    let mut cursor_x = x;
    let mut cursor_y = y;
    for ch in text.chars() {
        if ch == '\n' {
            cursor_x = x;
            cursor_y = cursor_y.saturating_add(10);
            continue;
        }
        let Some(glyph) = BASIC_FONTS.get(ch) else {
            cursor_x = cursor_x.saturating_add(8);
            continue;
        };
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (1 << col) != 0 {
                    fill_rect(buffer, width, height, Rect {
                        x: cursor_x + col, y: cursor_y + row as i32, width: 1, height: 1
                    }, color);
                }
            }
        }
        cursor_x = cursor_x.saturating_add(8);
        if cursor_x >= width as i32 { cursor_x = x; cursor_y = cursor_y.saturating_add(10); }
        if cursor_y >= height as i32 { break; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_css_color_forms_and_rejects_invalid_values() {
        assert_eq!(parse_css_color("red"), Some(0x00FF0000));
        assert_eq!(parse_css_color("#0fA"), Some(0x0000FFAA));
        assert_eq!(parse_css_color("#123456"), Some(0x00123456));
        assert_eq!(parse_css_color("rgb(0, 128, 255)"), Some(0x000080FF));
        assert_eq!(parse_css_color("rgb(256,0,0)"), None);
        assert_eq!(parse_css_color("#12"), None);
        assert_eq!(parse_css_color("nonsense"), None);
    }

    #[test]
    fn later_paint_commands_cover_earlier_commands() {
        let commands = [
            PaintCommand::FillRect { rect: Rect { x: 0, y: 0, width: 2, height: 2 }, color: 1 },
            PaintCommand::FillRect { rect: Rect { x: 1, y: 1, width: 1, height: 1 }, color: 2 },
        ];
        assert_eq!(rasterize(2, 2, 0, &commands)[3], 2);
    }

    #[test]
    fn fills_rectangle_pixels_and_clips_to_canvas() {
        let commands = [PaintCommand::FillRect {
            rect: Rect { x: 2, y: 1, width: 3, height: 2 }, color: 0x0000FF00
        }];
        let pixels = rasterize(5, 4, 0x00000000, &commands);
        assert_eq!(pixels[1 * 5 + 2], 0x0000FF00);
        assert_eq!(pixels[2 * 5 + 4], 0x0000FF00);
        assert_eq!(pixels[0], 0x00000000);
    }

    #[test]
    fn negative_rectangle_coordinates_are_clipped() {
        let commands = [PaintCommand::FillRect {
            rect: Rect { x: -2, y: -1, width: 4, height: 3 }, color: 0x000000FF
        }];
        let pixels = rasterize(3, 3, 0, &commands);
        assert_eq!(pixels[0], 0x000000FF);
        assert_eq!(pixels[2], 0);
    }

    #[test]
    fn rasterizes_scaled_image_pixels() {
        let image = std::sync::Arc::new(DecodedImage { width: 1, height: 1, pixels: vec![0x00123456] });
        let commands = [PaintCommand::Image { rect: Rect { x: 1, y: 1, width: 2, height: 2 }, image }];
        let pixels = rasterize(4, 4, 0, &commands);
        assert_eq!(pixels[1 * 4 + 1], 0x00123456);
        assert_eq!(pixels[2 * 4 + 2], 0x00123456);
        assert_eq!(pixels[0], 0);
    }

    #[test]
    fn layout_produces_paint_commands() {
        let dom = crate::html::parse("<p>Paint me</p>");
        let rules = crate::css::parse_stylesheet("");
        let styles = crate::css::compute_document_styles(&dom, &rules);
        let layout = crate::layout::layout_document(&dom, &styles, crate::layout::Viewport { width: 100, height: 100 });
        let commands = build_paint_list(&layout);
        assert!(commands.iter().any(|c| matches!(c, PaintCommand::Text { text, .. } if text.contains("Paint me"))));
    }
}
