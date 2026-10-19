//! URL resolution policy for browser navigation. Only HTTP and HTTPS are allowed.


/// Minimal in-memory session history. Only successfully loaded final URLs should be committed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<String>,
    index: Option<usize>,
}

impl History {
    pub fn current_index(&self) -> Option<usize> { self.index }
    pub fn current(&self) -> Option<&str> { self.index.and_then(|i| self.entries.get(i)).map(String::as_str) }
    pub fn back(&mut self) -> Option<String> {
        let i = self.index?;
        if i == 0 { return None; }
        self.index = Some(i - 1);
        self.current().map(str::to_owned)
    }
    pub fn forward(&mut self) -> Option<String> {
        let i = self.index?;
        if i + 1 >= self.entries.len() { return None; }
        self.index = Some(i + 1);
        self.current().map(str::to_owned)
    }
    pub fn commit_success(&mut self, url: &str) {
        if self.current() == Some(url) { return; }
        if let Some(i) = self.index { self.entries.truncate(i + 1); }
        self.entries.push(url.to_owned());
        self.index = Some(self.entries.len() - 1);
    }
}

/// Resolve a link target against the current document URL, rejecting unsupported schemes.
pub fn resolve_http_target(base: &str, target: &str) -> Result<String, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("empty link target".to_owned());
    }
    let base_url = url::Url::parse(base).map_err(|e| format!("invalid base URL: {e}"))?;
    if !matches!(base_url.scheme(), "http" | "https") {
        return Err(format!("unsupported base URL scheme '{}'", base_url.scheme()));
    }
    let resolved = base_url.join(target).map_err(|e| format!("invalid link target: {e}"))?;
    if !matches!(resolved.scheme(), "http" | "https") {
        return Err(format!("unsupported link scheme '{}'; only HTTP and HTTPS are supported", resolved.scheme()));
    }
    if resolved.host_str().is_none() {
        return Err("link URL has no host".to_owned());
    }
    Ok(resolved.to_string())
}

#[cfg(test)]
mod tests {
    use super::{resolve_http_target, History};

    #[test]
    fn history_back_forward_and_branching_are_deterministic() {
        let mut history = History::default();
        assert_eq!(history.back(), None);
        history.commit_success("https://example.com/a");
        history.commit_success("https://example.com/b");
        assert_eq!(history.back().as_deref(), Some("https://example.com/a"));
        assert_eq!(history.forward().as_deref(), Some("https://example.com/b"));
        assert_eq!(history.forward(), None);
        history.back();
        history.commit_success("https://example.com/c");
        assert_eq!(history.forward(), None);
        assert_eq!(history.current(), Some("https://example.com/c"));
    }

    #[test]
    fn repeated_current_url_does_not_duplicate_history() {
        let mut history = History::default();
        history.commit_success("https://example.com/");
        history.commit_success("https://example.com/");
        assert_eq!(history.entries.len(), 1);
    }

    #[test]
    fn resolves_relative_root_and_fragment_links() {
        let base = "https://example.com/dir/page.html";
        assert_eq!(resolve_http_target(base, "next.html").unwrap(), "https://example.com/dir/next.html");
        assert_eq!(resolve_http_target(base, "/home").unwrap(), "https://example.com/home");
        assert_eq!(resolve_http_target(base, "#part").unwrap(), "https://example.com/dir/page.html#part");
    }

    #[test]
    fn rejects_non_http_schemes_and_empty_targets() {
        assert!(resolve_http_target("https://example.com/", "javascript:alert(1)").is_err());
        assert!(resolve_http_target("https://example.com/", "data:text/html,hello").is_err());
        assert!(resolve_http_target("https://example.com/", "  ").is_err());
    }

    #[test]
    fn rejects_invalid_base_urls() {
        assert!(resolve_http_target("not a url", "/path").is_err());
        assert!(resolve_http_target("file:///tmp/page.html", "/path").is_err());
    }
}
