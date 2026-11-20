pub(super) struct MathParser<'a> {
    pub(super) input: &'a str,
    pub(super) pos: usize,
    pub(super) has_binary_op: bool,
    pub(super) depth: usize,
}

impl<'a> MathParser<'a> {
    pub(super) const fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            has_binary_op: false,
            depth: 0,
        }
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    pub(super) fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    pub(super) fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    pub(super) fn parse_expr(&mut self) -> Option<f64> {
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

    pub(super) fn parse_term(&mut self) -> Option<f64> {
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

    pub(super) fn parse_power(&mut self) -> Option<f64> {
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

    pub(super) fn parse_unary(&mut self) -> Option<f64> {
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

    pub(super) fn parse_primary(&mut self) -> Option<f64> {
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

    pub(super) fn parse_number(&mut self) -> Option<f64> {
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

pub(super) fn format_math_result(value: f64) -> Option<String> {
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

pub(super) fn insert_thousands_commas(digits: &str) -> String {
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
