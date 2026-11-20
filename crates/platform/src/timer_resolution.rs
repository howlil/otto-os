use std::sync::atomic::{AtomicU32, Ordering};

const FINE_RESOLUTION_MS: u32 = 1;

#[link(name = "winmm")]
unsafe extern "system" {
    fn timeBeginPeriod(period: u32) -> u32;
    fn timeEndPeriod(period: u32) -> u32;
}

static HOLDERS: AtomicU32 = AtomicU32::new(0);

pub fn begin_fine_timing() {
    if HOLDERS.fetch_add(1, Ordering::AcqRel) == 0 {
        unsafe {
            timeBeginPeriod(FINE_RESOLUTION_MS);
        }
    }
}

pub fn end_fine_timing() {
    let released = HOLDERS.fetch_update(Ordering::AcqRel, Ordering::Acquire, |held| {
        held.checked_sub(1)
    });
    if released == Ok(1) {
        unsafe {
            timeEndPeriod(FINE_RESOLUTION_MS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbalanced_end_never_underflows_and_balanced_pairs_release() {
        end_fine_timing();
        assert_eq!(HOLDERS.load(Ordering::Acquire), 0);
        begin_fine_timing();
        begin_fine_timing();
        end_fine_timing();
        assert_eq!(HOLDERS.load(Ordering::Acquire), 1);
        end_fine_timing();
        assert_eq!(HOLDERS.load(Ordering::Acquire), 0);
    }
}
