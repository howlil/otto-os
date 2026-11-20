use crate::layout::{Rect, scale};

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

fn clamp_cursor_byte(query: &str, cursor: usize) -> usize {
    let mut pos = cursor.min(query.len());
    while pos > 0 && !query.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

fn sanitize_single_line_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else if !ch.is_control() {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

struct MathParser<'a> {
    input: &'a str,
    pos: usize,
    has_binary_op: bool,
    depth: usize,
}

impl<'a> MathParser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            has_binary_op: false,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn parse_expr(&mut self) -> Option<f64> {
        if self.depth >= 64 {
            return None;
        }
        self.depth += 1;
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.bump();
                    self.has_binary_op = true;
                    value += self.parse_term()?;
                }
                Some('-') => {
                    self.bump();
                    self.has_binary_op = true;
                    value -= self.parse_term()?;
                }
                _ => break,
            }
            if !value.is_finite() {
                self.depth -= 1;
                return None;
            }
        }
        self.depth -= 1;
        Some(value)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut value = self.parse_power()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*' | '×') => {
                    self.bump();
                    self.has_binary_op = true;
                    value *= self.parse_power()?;
                }
                Some('/' | '÷') => {
                    self.bump();
                    self.has_binary_op = true;
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return None;
                    }
                    value /= rhs;
                }
                Some('%') => {
                    self.bump();
                    self.has_binary_op = true;
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return None;
                    }
                    value %= rhs;
                }
                _ => break,
            }
            if !value.is_finite() {
                return None;
            }
        }
        Some(value)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_unary()?;
        self.skip_ws();
        if self.peek() == Some('^') {
            self.bump();
            self.has_binary_op = true;
            let exp = self.parse_power()?;
            let value = base.powf(exp);
            if !value.is_finite() {
                return None;
            }
            return Some(value);
        }
        Some(base)
    }

    fn parse_unary(&mut self) -> Option<f64> {
        self.skip_ws();
        match self.peek() {
            Some('+') => {
                self.bump();
                if self.depth >= 64 {
                    return None;
                }
                self.depth += 1;
                let value = self.parse_unary();
                self.depth -= 1;
                value
            }
            Some('-') => {
                self.bump();
                if self.depth >= 64 {
                    return None;
                }
                self.depth += 1;
                let value = self.parse_unary().map(|inner| -inner);
                self.depth -= 1;
                value
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Option<f64> {
        self.skip_ws();
        if self.peek() == Some('(') {
            self.bump();
            let value = self.parse_expr()?;
            self.skip_ws();
            if self.bump() != Some(')') {
                return None;
            }
            return Some(value);
        }
        self.parse_number()
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let first = self.peek()?;
        if !first.is_ascii_digit() && first != '.' {
            return None;
        }
        let mut buf = String::new();
        let mut saw_digit = false;
        let mut dot_count = 0usize;
        let mut last_was_sep = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                saw_digit = true;
                last_was_sep = false;
                buf.push(ch);
                self.bump();
            } else if ch == '.' {
                dot_count += 1;
                if dot_count > 1 || last_was_sep {
                    return None;
                }
                last_was_sep = false;
                buf.push('.');
                self.bump();
            } else if ch == ',' || ch == '_' {
                if !saw_digit || last_was_sep {
                    return None;
                }
                last_was_sep = true;
                self.bump();
            } else {
                break;
            }
        }
        if !saw_digit || last_was_sep {
            return None;
        }
        let value: f64 = buf.parse().ok()?;
        value.is_finite().then_some(value)
    }
}

fn format_math_result(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let normalized = if value == 0.0 { 0.0 } else { value };
    if normalized.abs() >= 1e15 {
        return Some(format!("{normalized}"));
    }
    let formatted = format!("{normalized:.6}");
    let (int_part, frac_part) = formatted.split_once('.')?;
    let trimmed_frac = frac_part.trim_end_matches('0');
    let negative = int_part.starts_with('-');
    let digits = int_part.strip_prefix('-').unwrap_or(int_part);
    if digits == "0" && trimmed_frac.is_empty() {
        return Some(String::from("0"));
    }
    let with_commas = insert_thousands_commas(digits);
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&with_commas);
    if !trimmed_frac.is_empty() {
        out.push('.');
        out.push_str(trimmed_frac);
    }
    Some(out)
}

fn insert_thousands_commas(digits: &str) -> String {
    if digits.len() <= 3 {
        return digits.to_string();
    }
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx.is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpotlightView {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightHit {
    SearchField,
    SearchTab,
    AppsTab,
    GridView,
    ListView,
    Result(usize),
    Drag,
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotlightMove {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotlightResultLayout {
    pub index: usize,
    pub tile: Rect,
    pub icon: Rect,
    pub label: Rect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpotlightLayout {
    pub window: Rect,
    pub search_field: Rect,
    pub search_tab: Rect,
    pub apps_tab: Rect,
    pub grid_view: Rect,
    pub list_view: Rect,
    pub drag_handle: Rect,
    pub resize_handle: Rect,
    pub empty_state: Rect,
    pub results: Vec<SpotlightResultLayout>,
    pub columns: usize,
    pub view: SpotlightView,
    pub corner_radius: f32,
}

impl SpotlightLayout {
    #[must_use]
    pub fn hit_test(&self, x: i32, y: i32) -> Option<SpotlightHit> {
        if self.resize_handle.contains(x, y) {
            return Some(SpotlightHit::Resize);
        }
        if self.search_field.contains(x, y) {
            return Some(SpotlightHit::SearchField);
        }
        if self.search_tab.contains(x, y) {
            return Some(SpotlightHit::SearchTab);
        }
        if self.apps_tab.contains(x, y) {
            return Some(SpotlightHit::AppsTab);
        }
        if self.grid_view.contains(x, y) {
            return Some(SpotlightHit::GridView);
        }
        if self.list_view.contains(x, y) {
            return Some(SpotlightHit::ListView);
        }
        self.results
            .iter()
            .find(|result| result.tile.contains(x, y))
            .map(|result| SpotlightHit::Result(result.index))
            .or_else(|| {
                self.drag_handle
                    .contains(x, y)
                    .then_some(SpotlightHit::Drag)
            })
    }
}

#[must_use]
pub fn compute_spotlight_layout(
    origin: (i32, i32),
    width: i32,
    height: i32,
    dpi: u32,
    view: SpotlightView,
    result_count: usize,
    first_visible: usize,
) -> SpotlightLayout {
    let width = width.max(scale(420, dpi));
    let height = height.max(scale(300, dpi));
    let inset = scale(16, dpi);
    let search_height = scale(44, dpi);
    let tab_height = scale(30, dpi);
    let header_top = scale(18, dpi);
    let tabs_top = header_top + search_height + scale(12, dpi);
    let results_top = tabs_top + tab_height + scale(12, dpi);
    let results_bottom = height - scale(34, dpi);
    let results_height = (results_bottom - results_top).max(0);
    let content_width = (width - inset * 2).max(0);
    let search_field = Rect {
        x: inset,
        y: header_top,
        width: content_width,
        height: search_height,
    };
    let search_tab = Rect {
        x: inset,
        y: tabs_top,
        width: scale(76, dpi),
        height: tab_height,
    };
    let apps_tab = Rect {
        x: search_tab.right() + scale(6, dpi),
        y: tabs_top,
        width: scale(76, dpi),
        height: tab_height,
    };
    let list_width = scale(42, dpi);
    let view_gap = scale(4, dpi);
    let list_view = Rect {
        x: width - inset - list_width,
        y: tabs_top,
        width: list_width,
        height: tab_height,
    };
    let grid_view = Rect {
        x: list_view.x - view_gap - list_width,
        y: tabs_top,
        width: list_width,
        height: tab_height,
    };
    let (columns, results) = spotlight_result_layouts(
        view,
        SpotlightResultsRegion {
            width,
            dpi,
            inset,
            content_width,
            top: results_top,
            height: results_height,
        },
        result_count,
        first_visible,
    );
    let resize_size = scale(18, dpi);
    SpotlightLayout {
        window: Rect {
            x: origin.0,
            y: origin.1,
            width,
            height,
        },
        search_field,
        search_tab,
        apps_tab,
        grid_view,
        list_view,
        drag_handle: Rect {
            x: 0,
            y: 0,
            width,
            height: results_top,
        },
        resize_handle: Rect {
            x: width - resize_size,
            y: height - resize_size,
            width: resize_size,
            height: resize_size,
        },
        empty_state: Rect {
            x: inset,
            y: results_top,
            width: content_width,
            height: results_height,
        },
        results,
        columns,
        view,
        corner_radius: scale(28, dpi) as f32,
    }
}

#[derive(Clone, Copy)]
struct SpotlightResultsRegion {
    width: i32,
    dpi: u32,
    inset: i32,
    content_width: i32,
    top: i32,
    height: i32,
}

fn spotlight_result_layouts(
    view: SpotlightView,
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    match view {
        SpotlightView::Grid => grid_result_layouts(region, result_count, first_visible),
        SpotlightView::List => list_result_layouts(region, result_count, first_visible),
    }
}

fn grid_result_layouts(
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    let columns = 6;
    let start = first_visible.min(result_count) / columns * columns;
    let gap = scale(8, region.dpi);
    let cell_width = (region.content_width - gap * (columns as i32 - 1)).max(0) / columns as i32;
    let row_height = scale(108, region.dpi);
    let visible_rows = (region.height / row_height.max(1)).max(1) as usize;
    let end = start
        .saturating_add(visible_rows.saturating_mul(columns))
        .min(result_count);
    let results = (start..end)
        .map(|index| {
            let offset = index - start;
            let column = offset % columns;
            let row = offset / columns;
            let x = region.inset + column as i32 * (cell_width + gap);
            let y = region.top + row as i32 * row_height;
            let icon_size = scale(58, region.dpi).min((cell_width - scale(12, region.dpi)).max(0));
            let icon = Rect {
                x: x + (cell_width - icon_size) / 2,
                y: y + scale(9, region.dpi),
                width: icon_size,
                height: icon_size,
            };
            SpotlightResultLayout {
                index,
                tile: Rect {
                    x,
                    y,
                    width: cell_width,
                    height: row_height,
                },
                icon,
                label: Rect {
                    x: x + scale(4, region.dpi),
                    y: icon.bottom() + scale(4, region.dpi),
                    width: (cell_width - scale(8, region.dpi)).max(0),
                    height: scale(30, region.dpi),
                },
            }
        })
        .collect();
    (columns, results)
}

fn list_result_layouts(
    region: SpotlightResultsRegion,
    result_count: usize,
    first_visible: usize,
) -> (usize, Vec<SpotlightResultLayout>) {
    let start = first_visible.min(result_count);
    let row_height = scale(54, region.dpi);
    let visible_rows = (region.height / row_height.max(1)).max(1) as usize;
    let end = start.saturating_add(visible_rows).min(result_count);
    let results = (start..end)
        .map(|index| {
            let y = region.top + (index - start) as i32 * row_height;
            let icon_size = scale(36, region.dpi);
            let icon = Rect {
                x: region.inset + scale(10, region.dpi),
                y: y + (row_height - icon_size) / 2,
                width: icon_size,
                height: icon_size,
            };
            SpotlightResultLayout {
                index,
                tile: Rect {
                    x: region.inset,
                    y,
                    width: region.content_width,
                    height: row_height,
                },
                icon,
                label: Rect {
                    x: icon.right() + scale(12, region.dpi),
                    y: y + scale(4, region.dpi),
                    width: (region.width - region.inset - icon.right() - scale(12, region.dpi))
                        .max(0),
                    height: row_height - scale(8, region.dpi),
                },
            }
        })
        .collect();
    (1, results)
}

#[must_use]
pub fn rank_spotlight_results<'a>(
    names: impl IntoIterator<Item = &'a str>,
    query: &str,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut ranked: Vec<(u32, String, usize)> = names
        .into_iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let name_folded = name.to_lowercase();
            rank_name(&name_folded, &query).map(|score| (score, name_folded, index))
        })
        .collect();
    ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    ranked.into_iter().map(|(_, _, index)| index).collect()
}

fn rank_name(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let mut score = 0;
    for token in query.split_whitespace() {
        score += rank_token(name, token)?;
    }
    Some(score)
}

fn rank_token(name: &str, token: &str) -> Option<u32> {
    if name.starts_with(token) {
        return Some(0);
    }
    if name
        .split(|character: char| !character.is_alphanumeric())
        .any(|word| word.starts_with(token))
    {
        return Some(4);
    }
    if let Some(index) = name.find(token) {
        return Some(12 + u32::try_from(index).unwrap_or(u32::MAX).min(100));
    }
    let mut matched = 0;
    let mut gap = 0;
    for character in token.chars() {
        let remaining = &name[matched..];
        let offset = remaining.find(character)?;
        gap += offset;
        matched += offset + character.len_utf8();
    }
    Some(32 + u32::try_from(gap).unwrap_or(u32::MAX).min(100))
}

#[must_use]
pub fn move_spotlight_selection(
    current: Option<usize>,
    count: usize,
    view: SpotlightView,
    columns: usize,
    movement: SpotlightMove,
) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let columns = columns.max(1);
    let next = match current {
        Some(current) => {
            let current = current.min(count - 1);
            match movement {
                SpotlightMove::Up if view == SpotlightView::Grid => current.saturating_sub(columns),
                SpotlightMove::Down if view == SpotlightView::Grid => {
                    current.saturating_add(columns)
                }
                SpotlightMove::Left | SpotlightMove::Up => current.saturating_sub(1),
                SpotlightMove::Right | SpotlightMove::Down => current.saturating_add(1),
                SpotlightMove::Home => 0,
                SpotlightMove::End => count - 1,
            }
        }
        None => match movement {
            SpotlightMove::End => count - 1,
            _ => 0,
        },
    };
    Some(next.min(count - 1))
}

#[cfg(test)]
mod tests {
    use super::{
        SpotlightHit, SpotlightMode, SpotlightMove, SpotlightView, compute_spotlight_layout,
        move_spotlight_selection, rank_spotlight_results,
    };

    #[test]
    fn search_ranks_prefixes_before_word_and_substring_matches() {
        let names = [
            "Windows Calculator",
            "Calculator",
            "Character Map",
            "Notepad",
        ];
        assert_eq!(rank_spotlight_results(names, "calc"), vec![1, 0]);
        assert_eq!(rank_spotlight_results(names, "char"), vec![2]);
        assert!(rank_spotlight_results(names, "missing").is_empty());
    }

    #[test]
    fn grid_layout_bounds_visible_tiles_and_hit_targets() {
        let layout = compute_spotlight_layout((100, 40), 780, 600, 96, SpotlightView::Grid, 30, 0);
        assert!(layout.results.len() <= 30);
        assert!(layout.results.iter().all(|result| {
            result.tile.x >= 0
                && result.tile.y >= 0
                && result.tile.right() <= 780
                && result.tile.bottom() <= 600
        }));
        let first = layout.results.first().expect("visible result");
        assert_eq!(
            layout.hit_test(first.tile.x + 2, first.tile.y + 2),
            Some(SpotlightHit::Result(first.index))
        );
        assert_eq!(layout.window.x, 100);
    }

    #[test]
    fn list_layout_and_selection_moves_use_single_row_steps() {
        let layout = compute_spotlight_layout((0, 0), 780, 600, 96, SpotlightView::List, 100, 0);
        assert_eq!(layout.columns, 1);
        assert!(layout.results.len() < 100);
        assert_eq!(
            move_spotlight_selection(Some(1), 8, SpotlightView::List, 1, SpotlightMove::Down),
            Some(2)
        );
        assert_eq!(
            move_spotlight_selection(Some(4), 8, SpotlightView::Grid, 4, SpotlightMove::Down),
            Some(7)
        );
        assert_eq!(
            move_spotlight_selection(Some(0), 8, SpotlightView::Grid, 4, SpotlightMove::Up),
            Some(0)
        );
        assert_eq!(
            move_spotlight_selection(None, 0, SpotlightView::Grid, 6, SpotlightMove::Down),
            None
        );
    }

    #[test]
    fn navigation_without_a_selection_starts_at_the_first_result() {
        assert_eq!(
            move_spotlight_selection(None, 8, SpotlightView::Grid, 4, SpotlightMove::Down),
            Some(0)
        );
        assert_eq!(
            move_spotlight_selection(None, 8, SpotlightView::List, 1, SpotlightMove::End),
            Some(7)
        );
    }

    #[test]
    fn spotlight_modes_have_explicit_search_and_apps_states() {
        assert_eq!(SpotlightMode::default(), SpotlightMode::Search);
        assert_ne!(SpotlightMode::Apps, SpotlightMode::Search);
    }

    #[test]
    fn evaluate_math_handles_precedence_parens_power_and_formatting() {
        assert_eq!(
            SpotlightMode::evaluate_math("2 + 3 * 4").as_deref(),
            Some("14")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("(2 + 3) * 4").as_deref(),
            Some("20")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("2500 + 2500").as_deref(),
            Some("5,000")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("1,000 + 2_000").as_deref(),
            Some("3,000")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("5 / 2").as_deref(),
            Some("2.5")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("10 ÷ 4").as_deref(),
            Some("2.5")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("3 × 1000").as_deref(),
            Some("3,000")
        );
        assert_eq!(SpotlightMode::evaluate_math("10 % 3").as_deref(), Some("1"));
        assert_eq!(
            SpotlightMode::evaluate_math("2 ^ 3 ^ 2").as_deref(),
            Some("512")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("-5 + 12").as_deref(),
            Some("7")
        );
        assert_eq!(
            SpotlightMode::evaluate_math("0.1 + 0.2").as_deref(),
            Some("0.3")
        );
        assert_eq!(SpotlightMode::evaluate_math("1 - 1").as_deref(), Some("0"));
        assert_eq!(
            SpotlightMode::evaluate_math("1 - 1001").as_deref(),
            Some("-1,000")
        );
    }

    #[test]
    fn evaluate_math_rejects_plain_numbers_apps_and_invalid_expressions() {
        assert_eq!(SpotlightMode::evaluate_math(""), None);
        assert_eq!(SpotlightMode::evaluate_math("   "), None);
        assert_eq!(SpotlightMode::evaluate_math("42"), None);
        assert_eq!(SpotlightMode::evaluate_math("-42"), None);
        assert_eq!(SpotlightMode::evaluate_math("(42)"), None);
        assert_eq!(SpotlightMode::evaluate_math("7zip"), None);
        assert_eq!(SpotlightMode::evaluate_math("1password"), None);
        assert_eq!(SpotlightMode::evaluate_math("1 / 0"), None);
        assert_eq!(SpotlightMode::evaluate_math("1 % 0"), None);
        assert_eq!(SpotlightMode::evaluate_math("(-1) ^ 0.5"), None);
        assert_eq!(SpotlightMode::evaluate_math("2 + "), None);
        assert_eq!(SpotlightMode::evaluate_math("(2 + 3"), None);
        assert_eq!(SpotlightMode::evaluate_math("2 + 3)"), None);
    }

    #[test]
    fn caret_editing_inserts_deletes_and_navigates_utf8_safely() {
        let mut query = String::new();
        let mut cursor = 0;
        let mut selected_all = false;

        assert!(!SpotlightMode::insert_char(
            &mut query,
            &mut cursor,
            &mut selected_all,
            '\x01',
            10
        ));
        assert!(SpotlightMode::insert_char(
            &mut query,
            &mut cursor,
            &mut selected_all,
            'a',
            10
        ));
        assert!(SpotlightMode::insert_char(
            &mut query,
            &mut cursor,
            &mut selected_all,
            'é',
            10
        ));
        assert!(SpotlightMode::insert_char(
            &mut query,
            &mut cursor,
            &mut selected_all,
            'c',
            10
        ));
        assert_eq!(query, "aéc");
        assert_eq!(cursor, query.len());

        assert!(SpotlightMode::move_cursor_left(
            &query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(cursor, "aé".len());
        assert!(SpotlightMode::insert_char(
            &mut query,
            &mut cursor,
            &mut selected_all,
            'b',
            10
        ));
        assert_eq!(query, "aébc");
        assert_eq!(cursor, "aéb".len());

        assert!(SpotlightMode::backspace(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(query, "aéc");
        assert!(SpotlightMode::delete_forward(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(query, "aé");
        assert!(!SpotlightMode::delete_forward(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));

        assert!(SpotlightMode::move_cursor_home(
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(cursor, 0);
        assert!(!SpotlightMode::move_cursor_home(
            &mut cursor,
            &mut selected_all
        ));
        assert!(SpotlightMode::move_cursor_right(
            &query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(cursor, 1);
        assert!(SpotlightMode::move_cursor_end(
            &query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(cursor, query.len());
    }

    #[test]
    fn caret_editing_handles_select_all_word_delete_and_paste_sanitization() {
        let mut query = String::from("hello world");
        let mut cursor = query.len();
        let mut selected_all = false;

        assert!(SpotlightMode::delete_word_backward(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(query, "hello ");
        assert_eq!(cursor, 6);

        assert!(SpotlightMode::delete_word_backward(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(query, "");
        assert_eq!(cursor, 0);

        assert!(SpotlightMode::insert_text(
            &mut query,
            &mut cursor,
            &mut selected_all,
            "12\r\n+\t\x0734",
            20
        ));
        assert_eq!(query, "12 + 34");
        assert_eq!(cursor, 7);

        selected_all = true;
        assert!(SpotlightMode::move_cursor_left(
            &query,
            &mut cursor,
            &mut selected_all
        ));
        assert!(!selected_all);
        assert_eq!(cursor, 0);

        selected_all = true;
        assert!(SpotlightMode::insert_text(
            &mut query,
            &mut cursor,
            &mut selected_all,
            "abcdef",
            4
        ));
        assert_eq!(query, "abcd");
        assert_eq!(cursor, 4);
        assert!(!selected_all);

        selected_all = true;
        assert!(SpotlightMode::backspace(
            &mut query,
            &mut cursor,
            &mut selected_all
        ));
        assert_eq!(query, "");
        assert_eq!(cursor, 0);
        assert!(!selected_all);
    }

    #[test]
    fn spotlight_outer_shell_and_search_inset_follow_concentric_grid() {
        let layout = compute_spotlight_layout((0, 0), 780, 600, 96, SpotlightView::Grid, 0, 0);
        assert!((layout.corner_radius - 28.0).abs() < f32::EPSILON);
        assert_eq!(layout.search_field.x, 16);
        assert_eq!(layout.window.width - layout.search_field.right(), 16);
        assert_eq!(layout.corner_radius as i32 - layout.search_field.x, 12);

        let hidpi = compute_spotlight_layout((0, 0), 1170, 900, 144, SpotlightView::Grid, 0, 0);
        assert!((hidpi.corner_radius - 42.0).abs() < f32::EPSILON);
        assert_eq!(hidpi.search_field.x, 24);
        assert_eq!(hidpi.corner_radius as i32 - hidpi.search_field.x, 18);
    }
}
