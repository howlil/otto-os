use super::layout::SpotlightMove;
use super::layout::SpotlightView;

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

pub(super) fn rank_name(name: &str, query: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let mut score = 0;
    for token in query.split_whitespace() {
        score += rank_token(name, token)?;
    }
    Some(score)
}

pub(super) fn rank_token(name: &str, token: &str) -> Option<u32> {
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
