#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub timestamp_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LyricsSheet {
    pub lines: Vec<LyricLine>,
}

impl LyricsSheet {
    #[must_use]
    pub fn new(mut lines: Vec<LyricLine>) -> Self {
        lines.sort_by_key(|line| line.timestamp_ms);
        Self { lines }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn active_line_index(&self, position_ms: u64) -> Option<usize> {
        if self.lines.is_empty() || position_ms < self.lines[0].timestamp_ms {
            return None;
        }

        match self
            .lines
            .binary_search_by_key(&position_ms, |line| line.timestamp_ms)
        {
            Ok(idx) => Some(idx),
            Err(idx) => {
                if idx == 0 {
                    None
                } else {
                    Some(idx - 1)
                }
            }
        }
    }

    #[must_use]
    pub fn active_line(&self, position_ms: u64) -> Option<&str> {
        let idx = self.active_line_index(position_ms)?;
        Some(self.lines[idx].text.as_str())
    }

    #[must_use]
    pub fn visible_window(&self, position_ms: u64, window_size: usize) -> (usize, &[LyricLine]) {
        if self.lines.is_empty() || window_size == 0 {
            return (0, &[]);
        }

        let active = self.active_line_index(position_ms).unwrap_or(0);
        let half = window_size / 2;
        let start = active.saturating_sub(half);
        let end = (start + window_size).min(self.lines.len());
        let start = if end - start < window_size {
            end.saturating_sub(window_size)
        } else {
            start
        };

        (active.saturating_sub(start), &self.lines[start..end])
    }
}

fn parse_timestamp_ms(tag: &str) -> Option<u64> {
    let (min_str, sec_str) = tag.split_once(':')?;
    let mins: u64 = min_str.trim().parse().ok()?;

    let (secs_int, frac_ms) = if let Some((sec, frac)) = sec_str.split_once('.') {
        let secs: u64 = sec.trim().parse().ok()?;
        let frac_trimmed = frac.trim();
        let ms: u64 = match frac_trimmed.len() {
            0 => 0,
            1 => frac_trimmed.parse::<u64>().ok()? * 100,
            2 => frac_trimmed.parse::<u64>().ok()? * 10,
            _ => frac_trimmed[..3].parse::<u64>().ok()?,
        };
        (secs, ms)
    } else {
        let secs: u64 = sec_str.trim().parse().ok()?;
        (secs, 0)
    };

    Some(mins * 60_000 + secs_int * 1_000 + frac_ms)
}

#[must_use]
pub fn parse_lrc(content: &str) -> LyricsSheet {
    let mut collected = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }

        let mut remaining = trimmed;
        let mut timestamps = Vec::new();

        while remaining.starts_with('[') {
            if let Some(close_idx) = remaining.find(']') {
                let inside = &remaining[1..close_idx];
                if let Some(ms) = parse_timestamp_ms(inside) {
                    timestamps.push(ms);
                }
                remaining = remaining[close_idx + 1..].trim_start();
            } else {
                break;
            }
        }

        let text = remaining.trim().to_string();
        for ts in timestamps {
            collected.push(LyricLine {
                timestamp_ms: ts,
                text: text.clone(),
            });
        }
    }

    LyricsSheet::new(collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_lrc_timestamps() {
        let input = "\
[ti:Sample Song]
[ar:Artist Name]
[00:10.50]First line of lyrics
[00:20.00]Second line of lyrics
[01:05.250]Third line after one minute
";
        let sheet = parse_lrc(input);
        assert_eq!(sheet.len(), 3);
        assert_eq!(sheet.lines[0].timestamp_ms, 10_500);
        assert_eq!(sheet.lines[0].text, "First line of lyrics");
        assert_eq!(sheet.lines[1].timestamp_ms, 20_000);
        assert_eq!(sheet.lines[2].timestamp_ms, 65_250);
    }

    #[test]
    fn active_line_lookup_handles_bounds() {
        let sheet = LyricsSheet::new(vec![
            LyricLine {
                timestamp_ms: 10_000,
                text: "Verse 1".into(),
            },
            LyricLine {
                timestamp_ms: 25_000,
                text: "Chorus".into(),
            },
            LyricLine {
                timestamp_ms: 40_000,
                text: "Outro".into(),
            },
        ]);

        assert_eq!(sheet.active_line(5_000), None);
        assert_eq!(sheet.active_line(10_000), Some("Verse 1"));
        assert_eq!(sheet.active_line(15_000), Some("Verse 1"));
        assert_eq!(sheet.active_line(25_000), Some("Chorus"));
        assert_eq!(sheet.active_line(39_999), Some("Chorus"));
        assert_eq!(sheet.active_line(40_000), Some("Outro"));
        assert_eq!(sheet.active_line(99_999), Some("Outro"));
    }

    #[test]
    fn visible_window_slices_around_active_line() {
        let sheet = LyricsSheet::new(vec![
            LyricLine {
                timestamp_ms: 1_000,
                text: "L1".into(),
            },
            LyricLine {
                timestamp_ms: 2_000,
                text: "L2".into(),
            },
            LyricLine {
                timestamp_ms: 3_000,
                text: "L3".into(),
            },
            LyricLine {
                timestamp_ms: 4_000,
                text: "L4".into(),
            },
            LyricLine {
                timestamp_ms: 5_000,
                text: "L5".into(),
            },
        ]);

        let (active_rel, window) = sheet.visible_window(3_000, 3);
        assert_eq!(active_rel, 1);
        assert_eq!(window.len(), 3);
        assert_eq!(window[0].text, "L2");
        assert_eq!(window[1].text, "L3");
        assert_eq!(window[2].text, "L4");
    }

    #[test]
    fn handles_multiple_timestamps_and_out_of_order() {
        let input = "[00:30.00][00:10.00]Repeated line\n[00:20.00]Middle line";
        let sheet = parse_lrc(input);
        assert_eq!(sheet.len(), 3);
        assert_eq!(sheet.lines[0].timestamp_ms, 10_000);
        assert_eq!(sheet.lines[0].text, "Repeated line");
        assert_eq!(sheet.lines[1].timestamp_ms, 20_000);
        assert_eq!(sheet.lines[1].text, "Middle line");
        assert_eq!(sheet.lines[2].timestamp_ms, 30_000);
        assert_eq!(sheet.lines[2].text, "Repeated line");
    }
}
