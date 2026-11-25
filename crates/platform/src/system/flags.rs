use otto_core::{EXPERIMENTAL_FLAGS_ENV, ExperimentalFlags};

#[must_use]
pub fn read_experimental_flags() -> ExperimentalFlags {
    match std::env::var(EXPERIMENTAL_FLAGS_ENV) {
        Ok(raw) => ExperimentalFlags::parse(&raw),
        Err(_) => ExperimentalFlags::empty(),
    }
}

#[cfg(test)]
mod tests {
    use otto_core::{ExperimentalFlag, ExperimentalFlags};

    #[test]
    fn empty_default_flags_have_no_active_entries() {
        let flags = ExperimentalFlags::empty();
        assert!(flags.is_empty());
        assert!(!flags.is_enabled(ExperimentalFlag::CanaryDiagnostics));
    }
}
