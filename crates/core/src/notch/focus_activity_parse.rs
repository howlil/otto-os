use super::focus_activity::{MAX_SEARCH_QUERY_CHARS, normalize_domain, sanitize_label};

pub(super) fn extract_search_from_title(clean_title: &str) -> Option<(String, String)> {
    const PATTERNS: [(&str, &str); 6] = [
        (" - Google Search", "Google"),
        (" - Penelusuran Google", "Google"),
        (" - Bing", "Bing"),
        (" - Search", "Bing"),
        (" at DuckDuckGo", "DuckDuckGo"),
        (" - Brave Search", "Brave"),
    ];
    for (suffix, engine) in PATTERNS {
        if let Some(query) = clean_title.strip_suffix(suffix) {
            let normalized = sanitize_label(query, "", MAX_SEARCH_QUERY_CHARS);
            if !normalized.is_empty() {
                return Some((normalized, String::from(engine)));
            }
        }
    }
    None
}

pub(super) fn infer_domain_from_title(clean_title: &str) -> Option<String> {
    const KNOWN_TITLES: [(&str, &str); 15] = [
        ("youtube", "youtube.com"),
        ("reddit", "reddit.com"),
        ("github", "github.com"),
        ("stack overflow", "stackoverflow.com"),
        ("instagram", "instagram.com"),
        ("tiktok", "tiktok.com"),
        ("facebook", "facebook.com"),
        ("netflix", "netflix.com"),
        ("twitch", "twitch.tv"),
        ("chatgpt", "chatgpt.com"),
        ("claude", "claude.ai"),
        ("notion", "notion.so"),
        ("figma", "figma.com"),
        ("wikipedia", "wikipedia.org"),
        ("linkedin", "linkedin.com"),
    ];
    let lower = clean_title.to_ascii_lowercase();
    for (keyword, domain) in KNOWN_TITLES {
        if lower == keyword
            || lower.ends_with(&format!(" - {keyword}"))
            || lower.ends_with(&format!(": {keyword}"))
            || lower.starts_with(&format!("{keyword} - "))
            || lower.starts_with(&format!("{keyword}: "))
        {
            return Some(String::from(domain));
        }
    }
    if lower == "x"
        || lower.ends_with(" / x")
        || lower.ends_with(" - x")
        || lower.contains(" on x:")
    {
        return Some(String::from("x.com"));
    }
    for token in clean_title.split([' ', '|', '—', '·']) {
        if let Some(domain) = normalize_domain(token.trim_matches(|ch: char| {
            matches!(ch, '(' | ')' | '[' | ']' | ',' | ';' | ':' | '"' | '\'')
        })) {
            return Some(domain);
        }
    }
    None
}
