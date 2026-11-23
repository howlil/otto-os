const RELEASES_PAGE_PREFIX: &str = "https://github.com/howlil/otto-os/releases";
const RELEASES_DOWNLOAD_PREFIX: &str = "https://github.com/howlil/otto-os/releases/download/";
pub const RELEASE_CHECKSUMS_ASSET: &str = "SHA256SUMS.txt";
const MAX_JSON_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseUpdateInfo {
    pub version: String,
    pub tag: String,
    pub installer_name: String,
    pub installer_url: String,
    pub checksums_url: Option<String>,
    pub release_url: String,
}

impl ReleaseUpdateInfo {
    #[must_use]
    pub fn can_auto_install(&self) -> bool {
        !self.installer_name.is_empty()
            && is_allowed_release_asset_url(&self.installer_url, &self.tag, &self.installer_name)
            && self.checksums_url.as_deref().is_some_and(|url| {
                is_allowed_release_asset_url(url, &self.tag, RELEASE_CHECKSUMS_ASSET)
            })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate,
    NoReleases,
    Available(ReleaseUpdateInfo),
    Downloading {
        version: String,
        percent: Option<u8>,
    },
    Verifying {
        version: String,
    },
    Installing {
        version: String,
    },
    Failed {
        message: String,
        retry_available: Option<ReleaseUpdateInfo>,
    },
}

impl UpdateState {
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::Downloading { .. }
                | Self::Verifying { .. }
                | Self::Installing { .. }
        )
    }

    #[must_use]
    pub fn installable_release(&self) -> Option<&ReleaseUpdateInfo> {
        let info = match self {
            Self::Available(info)
            | Self::Failed {
                retry_available: Some(info),
                ..
            } => Some(info),
            _ => None,
        }?;
        info.can_auto_install().then_some(info)
    }

    #[must_use]
    pub fn release_page_url(&self) -> Option<&str> {
        match self {
            Self::Available(info)
            | Self::Failed {
                retry_available: Some(info),
                ..
            } if is_allowed_release_page_url(&info.release_url) => Some(&info.release_url),
            _ => None,
        }
    }

    #[must_use]
    pub fn status_label(&self) -> Option<String> {
        match self {
            Self::Idle => None,
            Self::Checking => Some(String::from(super::model::UPDATE_CHECKING_LABEL)),
            Self::UpToDate => Some(String::from("Up to date")),
            Self::NoReleases => Some(String::from("No releases yet")),
            Self::Available(info) => Some(format!("Update v{}", info.version)),
            Self::Downloading { version, percent } => Some(match percent {
                Some(pct) => format!("Downloading v{version} ({pct}%)"),
                None => format!("Downloading v{version}..."),
            }),
            Self::Verifying { version } => Some(format!("Verifying v{version}...")),
            Self::Installing { version } => Some(format!("Installing v{version}...")),
            Self::Failed { message, .. } => Some(message.clone()),
        }
    }
}

#[derive(Debug, Default)]
struct ParsedReleaseJson {
    tag_name: Option<String>,
    html_url: Option<String>,
    assets: Vec<(String, String)>,
}

#[must_use]
pub fn expected_installer_asset_name(tag: &str) -> Option<String> {
    let parsed = parse_semver(tag)?;
    Some(format!(
        "otto-os-setup-{}.{}.{}-x64.exe",
        parsed.core[0], parsed.core[1], parsed.core[2]
    ))
}

#[must_use]
pub fn is_allowed_release_asset_url(url: &str, tag: &str, asset_name: &str) -> bool {
    if !is_safe_url_segment(tag) || !is_safe_url_segment(asset_name) {
        return false;
    }
    let expected = format!("{RELEASES_DOWNLOAD_PREFIX}{tag}/{asset_name}");
    url == expected
}

#[must_use]
pub fn is_allowed_release_page_url(url: &str) -> bool {
    url.starts_with(RELEASES_PAGE_PREFIX)
        && !url
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
}

fn is_safe_url_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.contains("..")
        && !segment.bytes().any(|b| {
            b.is_ascii_control()
                || b.is_ascii_whitespace()
                || matches!(b, b'/' | b'\\' | b'?' | b'#' | b'%' | b'@')
        })
}

#[must_use]
pub fn parse_release_update(
    current_version: &str,
    http_status: u16,
    body: Option<&str>,
) -> UpdateState {
    if http_status == 404 {
        return UpdateState::NoReleases;
    }
    if !(200..300).contains(&http_status) {
        return UpdateState::Failed {
            message: String::from("Could not reach GitHub Releases"),
            retry_available: None,
        };
    }
    let Some(parsed) = body.and_then(parse_release_json) else {
        return UpdateState::Failed {
            message: String::from("Invalid release metadata"),
            retry_available: None,
        };
    };
    let Some(tag) = parsed.tag_name else {
        return UpdateState::Failed {
            message: String::from("Invalid release metadata"),
            retry_available: None,
        };
    };
    let Some(ordering) = compare_semver(&tag, current_version) else {
        return UpdateState::Failed {
            message: String::from("Invalid release version"),
            retry_available: None,
        };
    };
    if !ordering.is_gt() {
        return UpdateState::UpToDate;
    }
    let version = tag.trim().trim_start_matches(['v', 'V']).to_owned();
    let release_url = parsed
        .html_url
        .filter(|url| is_allowed_release_page_url(url))
        .unwrap_or_else(|| String::from(RELEASES_PAGE_PREFIX));
    let expected_installer = expected_installer_asset_name(&tag).unwrap_or_default();
    let installer_url = parsed
        .assets
        .iter()
        .find(|(name, url)| {
            *name == expected_installer
                && is_allowed_release_asset_url(url, &tag, &expected_installer)
        })
        .map(|(_, url)| url.clone())
        .unwrap_or_default();
    let installer_name = if installer_url.is_empty() {
        String::new()
    } else {
        expected_installer
    };
    let checksums_url = parsed
        .assets
        .iter()
        .find(|(name, url)| {
            name == RELEASE_CHECKSUMS_ASSET
                && is_allowed_release_asset_url(url, &tag, RELEASE_CHECKSUMS_ASSET)
        })
        .map(|(_, url)| url.clone());
    UpdateState::Available(ReleaseUpdateInfo {
        version,
        tag,
        installer_name,
        installer_url,
        checksums_url,
        release_url,
    })
}

fn parse_release_json(json: &str) -> Option<ParsedReleaseJson> {
    let bytes = json.as_bytes();
    let mut index = skip_json_whitespace(bytes, 0);
    if bytes.get(index) != Some(&b'{') {
        return None;
    }
    index = skip_json_whitespace(bytes, index + 1);
    let mut parsed = ParsedReleaseJson::default();
    if bytes.get(index) == Some(&b'}') {
        return Some(parsed);
    }
    loop {
        let (key, after_key) = parse_json_string(json, index)?;
        index = skip_json_whitespace(bytes, after_key);
        if bytes.get(index) != Some(&b':') {
            return None;
        }
        index = skip_json_whitespace(bytes, index + 1);
        match key.as_str() {
            "tag_name" if bytes.get(index) == Some(&b'"') => {
                let (value, next) = parse_json_string(json, index)?;
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    parsed.tag_name = Some(trimmed.to_owned());
                }
                index = next;
            }
            "html_url" if bytes.get(index) == Some(&b'"') => {
                let (value, next) = parse_json_string(json, index)?;
                parsed.html_url = Some(value);
                index = next;
            }
            "assets" if bytes.get(index) == Some(&b'[') => {
                let (assets, next) = parse_assets_array(json, index)?;
                parsed.assets = assets;
                index = next;
            }
            _ => {
                index = skip_json_value(json, index, 0)?;
            }
        }
        index = skip_json_whitespace(bytes, index);
        match bytes.get(index) {
            Some(&b',') => index = skip_json_whitespace(bytes, index + 1),
            Some(&b'}') => return Some(parsed),
            _ => return None,
        }
    }
}

fn parse_assets_array(json: &str, start: usize) -> Option<(Vec<(String, String)>, usize)> {
    let bytes = json.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return None;
    }
    let mut index = skip_json_whitespace(bytes, start + 1);
    let mut assets = Vec::new();
    if bytes.get(index) == Some(&b']') {
        return Some((assets, index + 1));
    }
    loop {
        if bytes.get(index) == Some(&b'{') {
            let (asset, next) = parse_asset_object(json, index)?;
            if let Some(pair) = asset {
                assets.push(pair);
            }
            index = next;
        } else {
            index = skip_json_value(json, index, 1)?;
        }
        index = skip_json_whitespace(bytes, index);
        match bytes.get(index) {
            Some(&b',') => index = skip_json_whitespace(bytes, index + 1),
            Some(&b']') => return Some((assets, index + 1)),
            _ => return None,
        }
    }
}

fn parse_asset_object(json: &str, start: usize) -> Option<(Option<(String, String)>, usize)> {
    let bytes = json.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut index = skip_json_whitespace(bytes, start + 1);
    let mut name: Option<String> = None;
    let mut download_url: Option<String> = None;
    if bytes.get(index) == Some(&b'}') {
        return Some((None, index + 1));
    }
    loop {
        let (key, after_key) = parse_json_string(json, index)?;
        index = skip_json_whitespace(bytes, after_key);
        if bytes.get(index) != Some(&b':') {
            return None;
        }
        index = skip_json_whitespace(bytes, index + 1);
        match key.as_str() {
            "name" if bytes.get(index) == Some(&b'"') => {
                let (value, next) = parse_json_string(json, index)?;
                name = Some(value);
                index = next;
            }
            "browser_download_url" if bytes.get(index) == Some(&b'"') => {
                let (value, next) = parse_json_string(json, index)?;
                download_url = Some(value);
                index = next;
            }
            _ => {
                index = skip_json_value(json, index, 2)?;
            }
        }
        index = skip_json_whitespace(bytes, index);
        match bytes.get(index) {
            Some(&b',') => index = skip_json_whitespace(bytes, index + 1),
            Some(&b'}') => {
                let pair = match (name, download_url) {
                    (Some(n), Some(u)) if !n.is_empty() && !u.is_empty() => Some((n, u)),
                    _ => None,
                };
                return Some((pair, index + 1));
            }
            _ => return None,
        }
    }
}

fn skip_json_value(json: &str, start: usize, depth: usize) -> Option<usize> {
    if depth >= MAX_JSON_DEPTH {
        return None;
    }
    let bytes = json.as_bytes();
    let index = skip_json_whitespace(bytes, start);
    match *bytes.get(index)? {
        b'"' => parse_json_string(json, index).map(|(_, next)| next),
        b'{' => {
            let mut cursor = skip_json_whitespace(bytes, index + 1);
            if bytes.get(cursor) == Some(&b'}') {
                return Some(cursor + 1);
            }
            loop {
                let (_, after_key) = parse_json_string(json, cursor)?;
                cursor = skip_json_whitespace(bytes, after_key);
                if bytes.get(cursor) != Some(&b':') {
                    return None;
                }
                cursor = skip_json_value(json, cursor + 1, depth + 1)?;
                cursor = skip_json_whitespace(bytes, cursor);
                match bytes.get(cursor) {
                    Some(&b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
                    Some(&b'}') => return Some(cursor + 1),
                    _ => return None,
                }
            }
        }
        b'[' => {
            let mut cursor = skip_json_whitespace(bytes, index + 1);
            if bytes.get(cursor) == Some(&b']') {
                return Some(cursor + 1);
            }
            loop {
                cursor = skip_json_value(json, cursor, depth + 1)?;
                cursor = skip_json_whitespace(bytes, cursor);
                match bytes.get(cursor) {
                    Some(&b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
                    Some(&b']') => return Some(cursor + 1),
                    _ => return None,
                }
            }
        }
        _ => {
            let mut cursor = index;
            while cursor < bytes.len()
                && !bytes[cursor].is_ascii_whitespace()
                && !matches!(bytes[cursor], b',' | b'}' | b']')
            {
                cursor += 1;
            }
            (cursor > index).then_some(cursor)
        }
    }
}

#[must_use]
pub fn parse_latest_release_tag(json: &str) -> Option<String> {
    parse_release_json(json).and_then(|parsed| parsed.tag_name)
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
    match parse_release_update(current, http_status, body) {
        UpdateState::NoReleases => String::from("No releases yet"),
        UpdateState::UpToDate => String::from("Up to date"),
        UpdateState::Available(info) => format!("Update v{}", info.version),
        _ => String::from("Check failed"),
    }
}
