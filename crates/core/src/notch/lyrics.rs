#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub timestamp_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LyricsStatus {
    #[default]
    Idle,
    Loading,
    Ready,
    NotFound {
        searched_online: bool,
    },
    Failed,
}

impl LyricsStatus {
    #[must_use]
    pub const fn placeholder(self) -> &'static str {
        match self {
            Self::Idle | Self::Loading => "Loading lyrics\u{2026}",
            Self::Ready
            | Self::NotFound {
                searched_online: true,
            } => "No lyrics found",
            Self::NotFound {
                searched_online: false,
            } => "No saved lyrics \u{b7} turn on online lyrics",
            Self::Failed => "Lyrics failed to load \u{b7} try again later",
        }
    }

    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Idle | Self::Loading)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LyricsView<'a> {
    pub sheet: Option<&'a LyricsSheet>,
    pub status: LyricsStatus,
    pub animated: bool,
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
        self.lines
            .partition_point(|line| line.timestamp_ms <= position_ms)
            .checked_sub(1)
    }

    #[must_use]
    pub fn active_line(&self, position_ms: u64) -> Option<&str> {
        let idx = self.active_line_index(position_ms)?;
        Some(self.lines[idx].text.as_str())
    }

    #[must_use]
    pub fn scroll_position(&self, position_ms: u64) -> f32 {
        let Some(active) = self.active_line_index(position_ms) else {
            return 0.0;
        };
        if active >= 1 {
            let (start, end) = self.scroll_window(active);
            if position_ms < end {
                return (active - 1) as f32 + ease_window(position_ms, start, end);
            }
        }
        let next = active + 1;
        if next < self.lines.len() {
            let (start, end) = self.scroll_window(next);
            if position_ms > start {
                return active as f32 + ease_window(position_ms, start, end);
            }
        }
        active as f32
    }

    fn scroll_window(&self, step: usize) -> (u64, u64) {
        let target = self.lines[step].timestamp_ms;
        let prev = self.lines[step - 1].timestamp_ms;
        let prev_settle = if step == 1 {
            0
        } else {
            ((target - prev) / 2).min(LYRIC_SCROLL_SETTLE_MS)
        };
        let min_start = prev + prev_settle;
        let next_settle = self
            .lines
            .get(step + 1)
            .map_or(LYRIC_SCROLL_SETTLE_MS, |next| {
                ((next.timestamp_ms - target) / 2).min(LYRIC_SCROLL_SETTLE_MS)
            });
        let start = target.saturating_sub(LYRIC_SCROLL_LEAD_MS).max(min_start);
        let end = target + next_settle;
        (start, end)
    }

    #[must_use]
    pub fn next_line_delay_ms(&self, position_ms: u64, playback_rate_permille: u32) -> Option<u64> {
        let next_idx = self
            .lines
            .partition_point(|line| line.timestamp_ms <= position_ms);
        let next = self.lines.get(next_idx)?;
        let media_wait = next.timestamp_ms.saturating_sub(position_ms).max(1);
        Some(crate::notch::media::media_wait_to_real_ms(
            media_wait,
            playback_rate_permille,
        ))
    }

    #[must_use]
    pub fn line_progress(&self, position_ms: u64) -> f32 {
        let Some(active) = self.active_line_index(position_ms) else {
            return 0.0;
        };
        let start = self.lines[active].timestamp_ms;
        let span = self
            .lines
            .get(active + 1)
            .map_or(LYRIC_LAST_LINE_MS, |next| {
                (next.timestamp_ms - start).min(LYRIC_FILL_MAX_MS)
            })
            .max(1);
        ((position_ms - start) as f32 / span as f32).clamp(0.0, 1.0)
    }
}

fn ease_window(position_ms: u64, start: u64, end: u64) -> f32 {
    let span = end.saturating_sub(start);
    if span == 0 {
        return 1.0;
    }
    let t = (position_ms.saturating_sub(start) as f32 / span as f32).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub const LYRIC_SCROLL_LEAD_MS: u64 = 120;
pub const LYRIC_SCROLL_SETTLE_MS: u64 = 120;
pub const LYRIC_FILL_MAX_MS: u64 = 6_000;
pub const LYRIC_LAST_LINE_MS: u64 = 4_000;

pub const MAX_LRC_LINES: usize = 1_000;
pub const MAX_LRC_TOTAL_TEXT_BYTES: usize = 131_072;
pub const MAX_LRC_TIMESTAMPS_PER_LINE: usize = 16;

fn parse_timestamp_ms(tag: &str) -> Option<u64> {
    let (min_str, sec_str) = tag.split_once(':')?;
    let mins: u64 = min_str.trim().parse().ok()?;

    let (secs_int, frac_ms) = if let Some((sec, frac)) = sec_str.split_once('.') {
        let secs: u64 = sec.trim().parse().ok()?;
        let frac_trimmed = frac.trim();
        let mut digits = [0u32; 3];
        let mut count = 0;
        for ch in frac_trimmed.chars() {
            if !ch.is_ascii_digit() {
                return None;
            }
            if count < 3 {
                digits[count] = ch.to_digit(10)?;
            }
            count += 1;
        }
        let ms = match count {
            0 => 0,
            1 => u64::from(digits[0]) * 100,
            2 => u64::from(digits[0] * 10 + digits[1]) * 10,
            _ => u64::from(digits[0] * 100 + digits[1] * 10 + digits[2]),
        };
        (secs, ms)
    } else {
        let secs: u64 = sec_str.trim().parse().ok()?;
        (secs, 0)
    };

    mins.checked_mul(60_000)?
        .checked_add(secs_int.checked_mul(1_000)?)?
        .checked_add(frac_ms)
}

#[must_use]
pub fn parse_lrc(content: &str) -> LyricsSheet {
    let mut collected = Vec::new();
    let mut total_text_bytes = 0;

    for line in content.lines() {
        if collected.len() >= MAX_LRC_LINES || total_text_bytes >= MAX_LRC_TOTAL_TEXT_BYTES {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }

        let mut remaining = trimmed;
        let mut timestamps = Vec::new();

        while remaining.starts_with('[') {
            if let Some(close_idx) = remaining.find(']') {
                let inside = &remaining[1..close_idx];
                if timestamps.len() < MAX_LRC_TIMESTAMPS_PER_LINE
                    && let Some(ms) = parse_timestamp_ms(inside)
                {
                    timestamps.push(ms);
                }
                remaining = remaining[close_idx + 1..].trim_start();
            } else {
                break;
            }
        }

        let text = remaining.trim().to_string();
        for ts in timestamps {
            if collected.len() >= MAX_LRC_LINES
                || total_text_bytes.saturating_add(text.len()) > MAX_LRC_TOTAL_TEXT_BYTES
            {
                break;
            }
            total_text_bytes += text.len();
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
    use super::{
        LYRIC_FILL_MAX_MS, LYRIC_LAST_LINE_MS, LYRIC_SCROLL_LEAD_MS, LYRIC_SCROLL_SETTLE_MS,
        LyricLine, LyricsSheet, LyricsStatus,
    };

    #[test]
    fn lyrics_status_explains_why_no_lines_are_shown() {
        assert!(LyricsStatus::Loading.is_pending());
        assert!(!LyricsStatus::Failed.is_pending());
        assert_eq!(
            LyricsStatus::Failed.placeholder(),
            "Lyrics failed to load \u{b7} try again later"
        );
        assert_eq!(
            LyricsStatus::NotFound {
                searched_online: true
            }
            .placeholder(),
            "No lyrics found"
        );
        assert!(
            LyricsStatus::NotFound {
                searched_online: false
            }
            .placeholder()
            .contains("online")
        );
        assert_ne!(
            LyricsStatus::Loading.placeholder(),
            LyricsStatus::Failed.placeholder()
        );
    }

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
    fn active_line_is_the_last_line_started_when_timestamps_repeat() {
        let sheet = LyricsSheet::new(vec![
            LyricLine {
                timestamp_ms: 10_000,
                text: "Before".into(),
            },
            LyricLine {
                timestamp_ms: 20_000,
                text: "Duet A".into(),
            },
            LyricLine {
                timestamp_ms: 20_000,
                text: "Duet B".into(),
            },
            LyricLine {
                timestamp_ms: 30_000,
                text: "After".into(),
            },
        ]);

        assert_eq!(sheet.active_line_index(9_999), None);
        assert_eq!(sheet.active_line_index(20_000), Some(2));
        assert_eq!(sheet.active_line(25_000), Some("Duet B"));
        assert_eq!(sheet.active_line(30_000), Some("After"));
    }

    fn five_line_sheet() -> LyricsSheet {
        LyricsSheet::new(
            [1_000, 2_000, 3_000, 4_000, 20_000]
                .into_iter()
                .map(|timestamp_ms| LyricLine {
                    timestamp_ms,
                    text: format!("L{timestamp_ms}"),
                })
                .collect(),
        )
    }

    #[test]
    fn scroll_rests_on_the_first_line_before_any_line_is_active() {
        let sheet = five_line_sheet();
        assert!(sheet.scroll_position(0).abs() < f32::EPSILON);
        assert!(sheet.scroll_position(1_000).abs() < f32::EPSILON);
        assert!(sheet.scroll_position(1_500).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_anticipates_and_settles_smoothly_around_each_timestamp() {
        let sheet = five_line_sheet();
        assert!(sheet.scroll_position(2_000 - LYRIC_SCROLL_LEAD_MS).abs() < f32::EPSILON);
        let mid = sheet.scroll_position(2_000);
        assert!((mid - 0.875).abs() < 0.01);
        assert!((sheet.scroll_position(2_000 + LYRIC_SCROLL_SETTLE_MS) - 1.0).abs() < f32::EPSILON);
        assert!((sheet.scroll_position(3_500) - 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_never_moves_backwards_while_playing_forward() {
        let sheet = five_line_sheet();
        let mut last = 0.0;
        for position in (0..25_000).step_by(17) {
            let now = sheet.scroll_position(position);
            assert!(now + f32::EPSILON >= last);
            last = now;
        }

        let tight_and_duplicate = LyricsSheet::new(vec![
            LyricLine {
                timestamp_ms: 1_000,
                text: "A".into(),
            },
            LyricLine {
                timestamp_ms: 1_080,
                text: "B".into(),
            },
            LyricLine {
                timestamp_ms: 1_080,
                text: "C".into(),
            },
            LyricLine {
                timestamp_ms: 1_120,
                text: "D".into(),
            },
        ]);
        let mut prev = 0.0;
        for position in 900..=1_400 {
            let now = tight_and_duplicate.scroll_position(position);
            assert!(now + f32::EPSILON >= prev, "backward scroll at {position}");
            prev = now;
        }
    }

    #[test]
    fn next_line_delay_scales_with_playback_rate() {
        let sheet = five_line_sheet();
        assert_eq!(sheet.next_line_delay_ms(850, 1_000), Some(150));
        assert_eq!(sheet.next_line_delay_ms(1_750, 1_000), Some(250));
        assert_eq!(sheet.next_line_delay_ms(1_750, 2_000), Some(125));
        assert_eq!(sheet.next_line_delay_ms(20_000, 1_000), None);
    }

    #[test]
    fn line_progress_fills_across_the_gap_and_caps_long_gaps() {
        let sheet = five_line_sheet();
        assert!(sheet.line_progress(500).abs() < f32::EPSILON);
        assert!(sheet.line_progress(1_000).abs() < f32::EPSILON);
        assert!((sheet.line_progress(1_500) - 0.5).abs() < f32::EPSILON);
        assert!((sheet.line_progress(4_000 + LYRIC_FILL_MAX_MS) - 1.0).abs() < f32::EPSILON);
        assert!((sheet.line_progress(4_000 + LYRIC_FILL_MAX_MS / 2) - 0.5).abs() < f32::EPSILON);
        assert!((sheet.line_progress(20_000 + LYRIC_LAST_LINE_MS / 2) - 0.5).abs() < f32::EPSILON);
        assert!((sheet.line_progress(99_000) - 1.0).abs() < f32::EPSILON);
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

    #[test]
    fn rejects_malformed_unicode_timestamp_without_panic() {
        let input = "[00:00.éé]malformed line\n[00:01.00]valid line";
        let sheet = parse_lrc(input);
        assert_eq!(sheet.len(), 1);
        assert_eq!(sheet.lines[0].timestamp_ms, 1_000);
        assert_eq!(sheet.lines[0].text, "valid line");
    }

    #[test]
    fn caps_allocation_amplification_from_repeated_timestamps() {
        use std::fmt::Write;
        let mut bomb = String::new();
        for sec in 0..5000 {
            let _ = write!(bomb, "[00:{:02}.00]", sec % 60);
        }
        bomb.push_str("Long text repeated many times across timestamps");
        let sheet = parse_lrc(&bomb);
        assert!(sheet.len() <= MAX_LRC_TIMESTAMPS_PER_LINE);
    }

    #[test]
    fn caps_total_lines_and_text_bytes_across_the_document() {
        use std::fmt::Write;
        let mut many = String::new();
        for index in 0..(MAX_LRC_LINES * 3) {
            let _ = writeln!(many, "[{:02}:{:02}.00]line", index / 60, index % 60);
        }
        assert_eq!(parse_lrc(&many).len(), MAX_LRC_LINES);

        let wide = "x".repeat(4_096);
        let mut heavy = String::new();
        for index in 0..200 {
            let _ = writeln!(heavy, "[00:{:02}.00]{wide}", index % 60);
        }
        let total: usize = parse_lrc(&heavy)
            .lines
            .iter()
            .map(|line| line.text.len())
            .sum();
        assert!(total <= MAX_LRC_TOTAL_TEXT_BYTES);
    }

    #[test]
    fn survives_hostile_fragments_without_panicking() {
        let fragments = [
            "[",
            "]",
            "[]",
            "[:]",
            "[00:]",
            "[:00]",
            "[00:00.]",
            "[99999999999999999999:00.00]x",
            "[18446744073709551615:59.999]x",
            "[00:18446744073709551615.00]x",
            "[00:00.٣٤]x",
            "[00:00.00",
            "[[00:00.00]]x",
            "[00:00.00][",
            "[é:é.é]é",
            "[00:00.00]🎵",
        ];
        for fragment in fragments {
            let _ = parse_lrc(fragment);
        }
        let _ = parse_lrc(&fragments.join("\n"));
        assert!(parse_lrc("[18446744073709551615:59.999]x").is_empty());
    }
}
