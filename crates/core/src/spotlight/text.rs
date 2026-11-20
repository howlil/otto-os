pub(super) fn clamp_cursor_byte(query: &str, cursor: usize) -> usize {
    let mut pos = cursor.min(query.len());
    while pos > 0 && !query.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

pub(super) fn sanitize_single_line_text(text: &str) -> String {
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
