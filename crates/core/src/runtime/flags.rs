pub const EXPERIMENTAL_FLAGS_ENV: &str = "OTTO_EXPERIMENTAL_FLAGS";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExperimentalFlag {
    CanaryDiagnostics,
}

impl ExperimentalFlag {
    pub const ALL: &'static [Self] = &[Self::CanaryDiagnostics];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CanaryDiagnostics => "canary_diagnostics",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let normalized = raw.trim();
        if normalized.eq_ignore_ascii_case("canary_diagnostics")
            || normalized.eq_ignore_ascii_case("canary-diagnostics")
        {
            return Some(Self::CanaryDiagnostics);
        }
        None
    }

    const fn bit(self) -> u32 {
        match self {
            Self::CanaryDiagnostics => 1 << 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExperimentalFlags {
    bits: u32,
}

impl ExperimentalFlags {
    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Self {
        let mut flags = Self::empty();
        for token in raw.split([',', ';', ' ', '\t', '\r', '\n']) {
            if let Some(flag) = ExperimentalFlag::parse(token) {
                flags.bits |= flag.bit();
            }
        }
        flags
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    #[must_use]
    pub const fn is_enabled(self, flag: ExperimentalFlag) -> bool {
        (self.bits & flag.bit()) != 0
    }

    #[must_use]
    pub fn enabled_names(self) -> Vec<&'static str> {
        ExperimentalFlag::ALL
            .iter()
            .copied()
            .filter(|flag| self.is_enabled(*flag))
            .map(ExperimentalFlag::as_str)
            .collect()
    }

    #[must_use]
    pub fn format_enabled(self) -> String {
        self.enabled_names().join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::{ExperimentalFlag, ExperimentalFlags};

    #[test]
    fn empty_string_enables_no_flags() {
        let flags = ExperimentalFlags::parse("");
        assert!(flags.is_empty());
        assert!(!flags.is_enabled(ExperimentalFlag::CanaryDiagnostics));
        assert!(flags.enabled_names().is_empty());
        assert_eq!(flags.format_enabled(), "");
    }

    #[test]
    fn parses_known_flags_case_insensitively_and_ignores_unknown_tokens() {
        let flags = ExperimentalFlags::parse("unknown_flag, CANARY-DIAGNOSTICS ; bogus");
        assert!(!flags.is_empty());
        assert!(flags.is_enabled(ExperimentalFlag::CanaryDiagnostics));
        assert_eq!(flags.enabled_names(), vec!["canary_diagnostics"]);
        assert_eq!(flags.format_enabled(), "canary_diagnostics");
    }

    #[test]
    fn duplicate_flags_are_idempotent() {
        let flags = ExperimentalFlags::parse("canary_diagnostics,canary_diagnostics");
        assert_eq!(flags.enabled_names(), vec!["canary_diagnostics"]);
    }
}
