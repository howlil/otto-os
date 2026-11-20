#[must_use]
pub fn parse_latest_release_tag(json: &str) -> Option<String> {
    let bytes = json.as_bytes();
    let mut index = 0usize;
    let mut depth = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' | b'[' => {
                depth += 1;
                index += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b'"' => {
                let (key, next_index) = parse_json_string(json, index)?;
                index = skip_json_whitespace(bytes, next_index);
                if bytes.get(index) == Some(&b':') {
                    index = skip_json_whitespace(bytes, index + 1);
                    if depth == 1 && key == "tag_name" && bytes.get(index) == Some(&b'"') {
                        let (value, _) = parse_json_string(json, index)?;
                        return (!value.trim().is_empty()).then_some(value);
                    }
                }
            }
            _ => index += 1,
        }
    }
    None
}

pub(super) fn skip_json_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

pub(super) fn parse_json_string(input: &str, start_quote: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start_quote) != Some(&b'"') {
        return None;
    }
    let mut out = String::new();
    let mut index = start_quote + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Some((out, index + 1)),
            b'\\' => {
                index += 1;
                let escaped = *bytes.get(index)?;
                match escaped {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let hex = input.get(index + 1..index + 5)?;
                        let code = u32::from_str_radix(hex, 16).ok()?;
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                        }
                        index += 4;
                    }
                    other => out.push(char::from(other)),
                }
                index += 1;
            }
            byte if byte < 0x80 => {
                out.push(char::from(byte));
                index += 1;
            }
            _ => {
                let tail = &input[index..];
                let ch = tail.chars().next()?;
                out.push(ch);
                index += ch.len_utf8();
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedSemVer<'a> {
    pub(super) core: [u64; 3],
    pub(super) prerelease: Option<&'a str>,
}

pub(super) fn parse_semver(raw: &str) -> Option<ParsedSemVer<'_>> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    let without_build = trimmed.split('+').next()?;
    let (core_str, prerelease) = match without_build.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        Some((_, _)) => return None,
        None => (without_build, None),
    };
    let mut parts = core_str.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = match parts.next() {
        Some(value) => value.parse::<u64>().ok()?,
        None => 0,
    };
    if parts.next().is_some() {
        return None;
    }
    Some(ParsedSemVer {
        core: [major, minor, patch],
        prerelease,
    })
}

pub(super) fn compare_prerelease(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut left_parts = left.split('.');
    let mut right_parts = right.split('.');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a), Some(b)) => {
                let ordering = match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(na), Ok(nb)) => na.cmp(&nb),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                };
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
        }
    }
}

pub(super) fn compare_semver(candidate: &str, current: &str) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;

    let left = parse_semver(candidate)?;
    let right = parse_semver(current)?;
    let core_order = left.core.cmp(&right.core);
    if core_order != Ordering::Equal {
        return Some(core_order);
    }
    Some(match (left.prerelease, right.prerelease) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_prerelease(a, b),
    })
}

#[must_use]
pub fn format_update_status(current: &str, http_status: u16, body: Option<&str>) -> String {
    if http_status == 404 {
        return String::from("No releases yet");
    }
    if !(200..300).contains(&http_status) {
        return String::from("Check failed");
    }
    let Some(tag) = body.and_then(parse_latest_release_tag) else {
        return String::from("Check failed");
    };
    let Some(ordering) = compare_semver(&tag, current) else {
        return String::from("Check failed");
    };
    if ordering.is_gt() {
        let clean = tag.trim().trim_start_matches(['v', 'V']);
        format!("Update v{clean}")
    } else {
        String::from("Up to date")
    }
}
