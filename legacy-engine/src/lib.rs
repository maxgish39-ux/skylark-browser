//! Android JNI entry points for Independent Browser's custom rendering pipeline.
mod css;
mod html;
mod images;
mod layout;
mod navigation;
mod network;
mod paint;
mod render_bridge;

use jni::objects::{JObject, JString};
use jni::sys::{jint, jintArray, jstring};
use jni::JNIEnv;
use std::sync::{Mutex, OnceLock};

static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();
fn set_last_error(value: String) {
    if let Ok(mut slot) = LAST_ERROR.get_or_init(|| Mutex::new(String::new())).lock() { *slot = value; }
}
fn last_error() -> String {
    LAST_ERROR.get_or_init(|| Mutex::new(String::new())).lock().map(|v| v.clone()).unwrap_or_else(|_| "Unknown native error".to_owned())
}

fn collect_styles(node: &html::Node, base_url: &str, out: &mut String, seen: &mut std::collections::HashSet<String>, requests: &mut usize) {
    if node.name.eq_ignore_ascii_case("style") {
        out.push_str(&node.text_content()); out.push('\n');
    } else if node.name.eq_ignore_ascii_case("link") {
        let rel = node.attributes.get("rel").map(String::as_str).unwrap_or("");
        if rel.split_ascii_whitespace().any(|v| v.eq_ignore_ascii_case("stylesheet")) {
            if let Some(href) = node.attributes.get("href") {
                if *requests < 12 {
                    if let Ok(base) = url::Url::parse(base_url) {
                        if let Ok(url) = base.join(href) {
                            if matches!(url.scheme(), "http" | "https") && seen.insert(url.to_string()) {
                                *requests += 1;
                                if let Ok(response) = network::fetch_resource(url.as_str(), 256 * 1024) {
                                    let content_type = response.content_type.as_deref().unwrap_or("").to_ascii_lowercase();
                                    if (response.status / 100 == 2) && (content_type.is_empty() || content_type.contains("text/css") || content_type.contains("text/plain")) {
                                        let css = String::from_utf8_lossy(&response.body);
                                        if !css.as_bytes().contains(&0) { out.push_str(&css); out.push('\n'); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for child in &node.children { collect_styles(child, base_url, out, seen, requests); }
}

fn collect_images(node: &mut html::Node, base_url: &str, images: &mut std::collections::HashMap<String, std::sync::Arc<images::DecodedImage>>, seen: &mut std::collections::HashSet<String>, requests: &mut usize, total_bytes: &mut usize) {
    if node.name.eq_ignore_ascii_case("img") {
        if let Some(src) = node.attributes.get("src").cloned() {
            if let Ok(resolved) = images::resolve_image_url(base_url, &src) {
                node.attributes.insert("data-ib-resolved-src".to_owned(), resolved.clone());
                if seen.insert(resolved.clone()) && *requests < 20 && *total_bytes < 8 * 1024 * 1024 {
                    *requests += 1;
                    if let Ok(response) = network::fetch_resource(&resolved, (8 * 1024 * 1024).saturating_sub(*total_bytes).min(network::MAX_IMAGE_RESPONSE_BYTES)) {
                        if response.status / 100 == 2 {
                            *total_bytes += response.body.len();
                            if let Ok(decoded) = images::decode_image(&response.body) { images.insert(resolved, std::sync::Arc::new(decoded)); }
                        }
                    }
                }
            }
        }
    }
    for child in &mut node.children { collect_images(child, base_url, images, seen, requests, total_bytes); }
}

struct LoadedPage {
    layout: layout::LayoutBox,
    commands: Vec<paint::PaintCommand>,
    final_url: String,
    title: String,
}

static PAGE: OnceLock<Mutex<Option<LoadedPage>>> = OnceLock::new();
fn page_slot() -> &'static Mutex<Option<LoadedPage>> { PAGE.get_or_init(|| Mutex::new(None)) }

/// Tallest document (in layout pixels) the Android shell will rasterize.
const MAX_DOCUMENT_HEIGHT: i32 = 12_000;

/// Fetch, parse, style and lay out a page. Rasterization happens later, in slices.
fn load_page(url: &str, width: usize, viewport_height: usize) -> Result<LoadedPage, String> {
    if !(200..=1400).contains(&width) || viewport_height == 0 || viewport_height > 5000 { return Err("Unsupported viewport dimensions".to_owned()); }
    let page = network::fetch_page(url).map_err(|e| e.to_string())?;
    if page.status < 200 || page.status >= 400 { return Err(format!("HTTP {} {}", page.status, page.status_text)); }
    let source = String::from_utf8_lossy(&page.body);
    let html_type = page.content_type.as_deref().map(|v| v.to_ascii_lowercase().contains("html")).unwrap_or_else(|| {
        let s = source.trim_start().to_ascii_lowercase(); s.starts_with("<!doctype html") || s.starts_with("<html") || s.starts_with("<head") || s.starts_with("<body")
    });
    if !html_type { return Err("Skylark can only display HTML pages right now".to_owned()); }
    let mut document = html::parse(&source);
    let mut decoded = std::collections::HashMap::new();
    collect_images(&mut document, &page.final_url, &mut decoded, &mut std::collections::HashSet::new(), &mut 0, &mut 0);
    let mut styles = String::new();
    collect_styles(&document, &page.final_url, &mut styles, &mut std::collections::HashSet::new(), &mut 0);
    let rules = css::parse_stylesheet(&styles);
    let computed = css::compute_document_styles(&document, &rules);
    let layout = layout::layout_document_with_images(&document, &computed, layout::Viewport { width: width as i32, height: viewport_height as i32 }, &decoded);
    let commands = paint::build_paint_list(&layout);
    let title = document.find_first("title").map(|n| n.text_content().trim().to_owned()).unwrap_or_default();
    Ok(LoadedPage { layout, commands, final_url: page.final_url.clone(), title })
}

/// Rasterize the horizontal strip `[y, y + height)` of the page into ARGB pixels.
fn render_slice(page: &LoadedPage, y: i32, height: i32, width: usize) -> Vec<i32> {
    use paint::PaintCommand as P;
    let bottom = y.saturating_add(height);
    let mut shifted: Vec<P> = Vec::new();
    for command in &page.commands {
        match command {
            P::FillRect { rect, color } => {
                if rect.y.saturating_add(rect.height) > y && rect.y < bottom {
                    shifted.push(P::FillRect { rect: layout::Rect { x: rect.x, y: rect.y - y, width: rect.width, height: rect.height }, color: *color });
                }
            }
            P::BorderRect { rect, color } => {
                if rect.y.saturating_add(rect.height) > y && rect.y < bottom {
                    shifted.push(P::BorderRect { rect: layout::Rect { x: rect.x, y: rect.y - y, width: rect.width, height: rect.height }, color: *color });
                }
            }
            P::Text { x, y: ty, text, color } => {
                let lines = text.matches('\n').count() as i32 + 1;
                if *ty < bottom && ty.saturating_add(lines * 10 + 10) > y {
                    shifted.push(P::Text { x: *x, y: *ty - y, text: text.clone(), color: *color });
                }
            }
            P::Image { rect, image } => {
                if rect.y.saturating_add(rect.height) > y && rect.y < bottom {
                    shifted.push(P::Image { rect: layout::Rect { x: rect.x, y: rect.y - y, width: rect.width, height: rect.height }, image: image.clone() });
                }
            }
        }
    }
    let pixels = paint::rasterize(width, height as usize, 0x00FFFFFF, &shifted);
    pixels.into_iter().map(|p| (0xFF000000u32 | (p & 0x00FFFFFF)) as i32).collect()
}

fn string_to_jstring(env: &mut JNIEnv, value: &str) -> jstring {
    env.new_string(value).map(|v| v.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Loads and lays out a page. Returns the document height in layout pixels, or -1 on error.
#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeLoadPage(mut env: JNIEnv, _this: JObject, url: JString, width: jint, viewport_height: jint) -> jint {
    let url: String = match env.get_string(&url) { Ok(v) => v.into(), Err(e) => { set_last_error(format!("Invalid URL string: {e}")); return -1; } };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| load_page(&url, width.max(1) as usize, viewport_height.max(1) as usize)));
    match result {
        Ok(Ok(page)) => {
            let height = page.layout.rect.height.clamp(1, MAX_DOCUMENT_HEIGHT);
            *page_slot().lock().unwrap_or_else(|e| e.into_inner()) = Some(page);
            height
        }
        Ok(Err(e)) => { set_last_error(e); -1 }
        Err(_) => { set_last_error("The page engine stopped while rendering this page".to_owned()); -1 }
    }
}

/// Rasterizes a strip of the loaded page. Returns null on error.
#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeRenderSlice(env: JNIEnv, _this: JObject, y: jint, height: jint) -> jintArray {
    let y = y.max(0);
    let height = height.clamp(1, 4000);
    let guard = page_slot().lock().unwrap_or_else(|e| e.into_inner());
    let Some(page) = guard.as_ref() else { set_last_error("No page is loaded".to_owned()); return std::ptr::null_mut(); };
    let width = page.layout.rect.width.max(1) as usize;
    let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render_slice(page, y, height, width)));
    let pixels = match rendered { Ok(v) => v, Err(_) => { set_last_error("The page engine stopped while drawing this page".to_owned()); return std::ptr::null_mut(); } };
    match env.new_int_array(pixels.len() as jint) {
        Ok(array) => {
            if let Err(e) = env.set_int_array_region(&array, 0, &pixels) { set_last_error(format!("Could not transfer rendered pixels: {e}")); return std::ptr::null_mut(); }
            array.into_raw()
        }
        Err(e) => { set_last_error(format!("Could not allocate rendered image: {e}")); std::ptr::null_mut() }
    }
}

/// Returns the absolute http(s) URL of the link at a layout-space point, or null.
#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeLinkAt(mut env: JNIEnv, _this: JObject, x: jint, y: jint) -> jstring {
    let guard = page_slot().lock().unwrap_or_else(|e| e.into_inner());
    let Some(page) = guard.as_ref() else { return std::ptr::null_mut(); };
    let Some(target) = page.layout.link_at(x, y) else { return std::ptr::null_mut(); };
    if target.trim_start().starts_with('#') { return std::ptr::null_mut(); }
    match navigation::resolve_http_target(&page.final_url, target) {
        Ok(resolved) => string_to_jstring(&mut env, &resolved),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeGetTitle(mut env: JNIEnv, _this: JObject) -> jstring {
    let guard = page_slot().lock().unwrap_or_else(|e| e.into_inner());
    let title = guard.as_ref().map(|p| p.title.clone()).unwrap_or_default();
    string_to_jstring(&mut env, &title)
}

#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeGetFinalUrl(mut env: JNIEnv, _this: JObject) -> jstring {
    let guard = page_slot().lock().unwrap_or_else(|e| e.into_inner());
    let url = guard.as_ref().map(|p| p.final_url.clone()).unwrap_or_default();
    string_to_jstring(&mut env, &url)
}

#[no_mangle]
pub extern "system" fn Java_com_skylark_browser_MainActivity_nativeGetLastError(mut env: JNIEnv, _this: JObject) -> jstring {
    string_to_jstring(&mut env, &last_error())
}
