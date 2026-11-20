use super::math::MathParser;
use super::math::format_math_result;
use super::text::clamp_cursor_byte;
use super::text::sanitize_single_line_text;

pub const SPOTLIGHT_DEFAULT_WIDTH_POINTS: i32 = 780;
pub const SPOTLIGHT_DEFAULT_HEIGHT_POINTS: i32 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpotlightMode {
    #[default]
    Search,
    Apps,
}

impl SpotlightMode {
    #[must_use]
    pub fn evaluate_math(query: &str) -> Option<String> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut parser = MathParser::new(trimmed);
        let value = parser.parse_expr()?;
        parser.skip_ws();
        if parser.pos != trimmed.len() || !parser.has_binary_op {
            return None;
        }
        format_math_result(value)
    }

    pub fn insert_char(
        query: &mut String,
        cursor: &mut usize,
        selected_all: &mut bool,
        ch: char,
        max_chars: usize,
    ) -> bool {
        if ch.is_control() {
            return false;
        }
        if *selected_all {
            if max_chars == 0 {
                return false;
            }
            query.clear();
            query.push(ch);
            *cursor = query.len();
            *selected_all = false;
            return true;
        }
        if query.chars().count() >= max_chars {
            return false;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        query.insert(pos, ch);
        *cursor = pos + ch.len_utf8();
        true
    }

    pub fn insert_text(
        query: &mut String,
        cursor: &mut usize,
        selected_all: &mut bool,
        text: &str,
        max_chars: usize,
    ) -> bool {
        let sanitized = sanitize_single_line_text(text);
        if sanitized.is_empty() || max_chars == 0 {
            return false;
        }
        if *selected_all {
            let clipped: String = sanitized.chars().take(max_chars).collect();
            if clipped.is_empty() {
                return false;
            }
            *query = clipped;
            *cursor = query.len();
            *selected_all = false;
            return true;
        }
        let available = max_chars.saturating_sub(query.chars().count());
        if available == 0 {
            return false;
        }
        let to_insert: String = sanitized.chars().take(available).collect();
        if to_insert.is_empty() {
            return false;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        query.insert_str(pos, &to_insert);
        *cursor = pos + to_insert.len();
        true
    }

    pub fn backspace(query: &mut String, cursor: &mut usize, selected_all: &mut bool) -> bool {
        if *selected_all {
            *selected_all = false;
            *cursor = 0;
            if query.is_empty() {
                return false;
            }
            query.clear();
            return true;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        if pos == 0 {
            *cursor = 0;
            return false;
        }
        let prev = query[..pos]
            .char_indices()
            .next_back()
            .map_or(0, |(idx, _)| idx);
        query.remove(prev);
        *cursor = prev;
        true
    }

    pub fn delete_forward(query: &mut String, cursor: &mut usize, selected_all: &mut bool) -> bool {
        if *selected_all {
            *selected_all = false;
            *cursor = 0;
            if query.is_empty() {
                return false;
            }
            query.clear();
            return true;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        if pos >= query.len() {
            *cursor = pos;
            return false;
        }
        query.remove(pos);
        *cursor = pos;
        true
    }

    pub fn delete_word_backward(
        query: &mut String,
        cursor: &mut usize,
        selected_all: &mut bool,
    ) -> bool {
        if *selected_all {
            *selected_all = false;
            *cursor = 0;
            if query.is_empty() {
                return false;
            }
            query.clear();
            return true;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        if pos == 0 {
            *cursor = 0;
            return false;
        }
        let mut new_pos = pos;
        while let Some((idx, ch)) = query[..new_pos].char_indices().next_back() {
            if ch.is_whitespace() {
                new_pos = idx;
            } else {
                break;
            }
        }
        if let Some((_, ch)) = query[..new_pos].char_indices().next_back() {
            let word_char = ch.is_alphanumeric() || ch == '_';
            while let Some((idx, prev)) = query[..new_pos].char_indices().next_back() {
                let prev_is_word = prev.is_alphanumeric() || prev == '_';
                if prev.is_whitespace() || prev_is_word != word_char {
                    break;
                }
                new_pos = idx;
            }
        }
        query.drain(new_pos..pos);
        *cursor = new_pos;
        true
    }

    pub fn move_cursor_left(query: &str, cursor: &mut usize, selected_all: &mut bool) -> bool {
        if *selected_all {
            *selected_all = false;
            *cursor = 0;
            return true;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        if pos == 0 {
            let changed = *cursor != 0;
            *cursor = 0;
            return changed;
        }
        let prev = query[..pos]
            .char_indices()
            .next_back()
            .map_or(0, |(idx, _)| idx);
        *cursor = prev;
        true
    }

    pub fn move_cursor_right(query: &str, cursor: &mut usize, selected_all: &mut bool) -> bool {
        if *selected_all {
            *selected_all = false;
            *cursor = query.len();
            return true;
        }
        let pos = clamp_cursor_byte(query, *cursor);
        if pos >= query.len() {
            let changed = *cursor != pos;
            *cursor = pos;
            return changed;
        }
        if let Some(ch) = query[pos..].chars().next() {
            *cursor = pos + ch.len_utf8();
            return true;
        }
        false
    }

    pub fn move_cursor_home(cursor: &mut usize, selected_all: &mut bool) -> bool {
        if *selected_all || *cursor != 0 {
            *selected_all = false;
            *cursor = 0;
            return true;
        }
        false
    }

    pub fn move_cursor_end(query: &str, cursor: &mut usize, selected_all: &mut bool) -> bool {
        let end = query.len();
        if *selected_all || *cursor != end {
            *selected_all = false;
            *cursor = end;
            return true;
        }
        false
    }
}
