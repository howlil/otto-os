use std::collections::{HashMap, HashSet};

pub const HOG_SAMPLE_INTERVAL_SECS: u64 = 10;
pub const MEMORY_BUFFER_THRESHOLD_BYTES: u64 = 100 * 1024 * 1024;
pub const SAFE_TO_END_MEMORY_THRESHOLD_BYTES: u64 = 150 * 1024 * 1024;
const HOG_CPU_PERCENT_OF_CORE: u64 = 20;
const HOG_SUSTAINED_SAMPLES: u8 = 4;
const NEVER_THROTTLE: [&str; 3] = ["explorer.exe", "otto-os.exe", "dwm.exe"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessThrottleKind {
    #[default]
    None,
    AutoEcoQos,
    ManualEcoQos,
}

impl ProcessThrottleKind {
    #[must_use]
    pub const fn is_throttled(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessSafety {
    #[default]
    SafeBackground,
    ActiveWindow,
    SystemProtected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProcessAuditFilter {
    #[default]
    Background,
    Throttled,
    Protected,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtectionReason {
    OttoSelf,
    CriticalProcess,
    DesktopShell,
    WindowsKernelOrService,
    ElevatedOrSystemToken,
}

impl ProtectionReason {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::CriticalProcess => "Windows critical process",
            Self::WindowsKernelOrService => "Windows core service",
            Self::DesktopShell => "Windows desktop shell",
            Self::OttoSelf => "Otto OS surface",
            Self::ElevatedOrSystemToken => "Elevated or system-owned",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct RawProcessAuditSample {
    pub pid: u32,
    pub exe_name: String,
    pub exe_path: Option<String>,
    pub memory_bytes: u64,
    pub cpu_tenths_percent: u16,
    pub session_id: u32,
    pub is_critical: bool,
    pub controllable: bool,
    pub terminatable: bool,
    pub is_foreground: bool,
    pub has_visible_window: bool,
    pub throttle_kind: ProcessThrottleKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditedAppGroup {
    pub exe_name: String,
    pub display_name: String,
    pub exe_path: Option<String>,
    pub process_count: usize,
    pub primary_pid: u32,
    pub memory_bytes: u64,
    pub cpu_tenths_percent: u32,
    pub safety: ProcessSafety,
    pub has_visible_window: bool,
    pub protection: Option<ProtectionReason>,
    pub throttle_kind: ProcessThrottleKind,
    pub can_throttle: bool,
    pub can_terminate: bool,
}

fn is_essential_user_background_utility(exe_name_lower: &str) -> bool {
    matches!(
        exe_name_lower,
        "onedrive.exe"
            | "onedrive.sync.service.exe"
            | "googledrivefs.exe"
            | "dropbox.exe"
            | "icloudservices.exe"
            | "iclouddrive.exe"
            | "vpnui.exe"
            | "vpnagent.exe"
            | "wireguard.exe"
            | "tailscaled.exe"
            | "tailscale-ipn.exe"
            | "nordvpn-service.exe"
            | "protonvpn.exe"
            | "openvpn-gui.exe"
            | "nvcontainer.exe"
            | "nvdisplay.container.exe"
            | "amddow.exe"
            | "radeonsoftware.exe"
            | "igfxem.exe"
            | "realtekaudioservice.exe"
            | "wavesmaxxaudioserviceapplication.exe"
            | "dolbydaxapi.exe"
            | "lghub.exe"
            | "lghub_agent.exe"
            | "icue.exe"
            | "rzsynapse.exe"
            | "razer synapse 3.exe"
            | "armourycrate.usersessionhelper.exe"
            | "vmmemwsl"
            | "wslservice.exe"
            | "docker desktop.exe"
            | "com.docker.backend.exe"
    )
}

impl AuditedAppGroup {
    #[must_use]
    pub const fn is_memory_buffer(&self) -> bool {
        matches!(self.safety, ProcessSafety::SafeBackground)
            && !self.has_visible_window
            && self.memory_bytes >= MEMORY_BUFFER_THRESHOLD_BYTES
    }

    #[must_use]
    pub const fn is_cpu_hog(&self) -> bool {
        matches!(self.safety, ProcessSafety::SafeBackground) && self.cpu_tenths_percent >= 50
    }

    #[must_use]
    pub fn is_safe_to_end_recommended(&self) -> bool {
        self.safety == ProcessSafety::SafeBackground
            && self.can_terminate
            && !self.has_visible_window
            && self.memory_bytes >= SAFE_TO_END_MEMORY_THRESHOLD_BYTES
            && !is_essential_user_background_utility(&self.exe_name)
    }

    #[must_use]
    pub const fn diagnosis_label(&self) -> &'static str {
        match self.safety {
            ProcessSafety::SystemProtected => match self.protection {
                Some(reason) => reason.label(),
                None => "System protected",
            },
            ProcessSafety::ActiveWindow => "Active foreground app",
            ProcessSafety::SafeBackground => match self.throttle_kind {
                ProcessThrottleKind::AutoEcoQos => "Auto-throttled (EcoQoS)",
                ProcessThrottleKind::ManualEcoQos => "Throttled manually (EcoQoS)",
                ProcessThrottleKind::None => {
                    if self.is_cpu_hog() {
                        "High background CPU"
                    } else if self.is_memory_buffer() {
                        "Hidden memory buffer"
                    } else if !self.has_visible_window {
                        "Background process"
                    } else {
                        "Open in background"
                    }
                }
            },
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessAuditReport {
    pub total_processes: usize,
    pub background_memory_bytes: u64,
    pub background_group_count: usize,
    pub memory_buffer_count: usize,
    pub safe_to_end_count: usize,
    pub auto_throttled_count: usize,
    pub manual_throttled_count: usize,
    pub protected_group_count: usize,
    pub groups: Vec<AuditedAppGroup>,
}

#[must_use]
pub fn classify_process_protection(
    exe_name_lower: &str,
    pid: u32,
    session_id: u32,
    is_critical: bool,
    controllable: bool,
) -> Option<ProtectionReason> {
    if exe_name_lower == "otto-os.exe" {
        return Some(ProtectionReason::OttoSelf);
    }
    if is_critical
        || pid <= 4
        || matches!(
            exe_name_lower,
            "system"
                | "registry"
                | "memory compression"
                | "secure system"
                | "vmmem"
                | "csrss.exe"
                | "lsass.exe"
                | "smss.exe"
                | "wininit.exe"
                | "services.exe"
                | "winlogon.exe"
                | "fontdrvhost.exe"
                | "lsaiso.exe"
        )
    {
        return Some(ProtectionReason::CriticalProcess);
    }
    if matches!(
        exe_name_lower,
        "explorer.exe"
            | "dwm.exe"
            | "sihost.exe"
            | "taskhostw.exe"
            | "shellexperiencehost.exe"
            | "startmenuexperiencehost.exe"
            | "searchhost.exe"
            | "textinputhost.exe"
            | "ctfmon.exe"
            | "applicationframehost.exe"
            | "lockapp.exe"
    ) {
        return Some(ProtectionReason::DesktopShell);
    }
    if session_id == 0
        || matches!(
            exe_name_lower,
            "svchost.exe"
                | "spoolsv.exe"
                | "audiodg.exe"
                | "msmpeng.exe"
                | "securityhealthservice.exe"
                | "wudfhost.exe"
                | "searchindexer.exe"
                | "conhost.exe"
                | "dllhost.exe"
                | "runtimebroker.exe"
                | "dashost.exe"
                | "taskmgr.exe"
        )
    {
        return Some(ProtectionReason::WindowsKernelOrService);
    }
    if !controllable {
        return Some(ProtectionReason::ElevatedOrSystemToken);
    }
    None
}

#[must_use]
pub fn friendly_process_display_name(exe_name_lower: &str) -> String {
    match exe_name_lower {
        "system" => return String::from("Windows System"),
        "registry" => return String::from("Windows Registry"),
        "memory compression" => return String::from("Memory Compression"),
        "secure system" => return String::from("Secure System"),
        "svchost.exe" => return String::from("Windows Service Host"),
        "dwm.exe" => return String::from("Desktop Window Manager"),
        "explorer.exe" => return String::from("Windows Explorer"),
        "csrss.exe" => return String::from("Client Server Runtime"),
        "lsass.exe" => return String::from("Local Security Authority"),
        "services.exe" => return String::from("Service Control Manager"),
        "wininit.exe" => return String::from("Windows Startup"),
        "winlogon.exe" => return String::from("Windows Logon"),
        "smss.exe" => return String::from("Session Manager"),
        "fontdrvhost.exe" => return String::from("Font Driver Host"),
        "msmpeng.exe" => return String::from("Microsoft Defender"),
        "securityhealthservice.exe" => return String::from("Windows Security Service"),
        "searchindexer.exe" => return String::from("Windows Search Indexer"),
        "searchhost.exe" => return String::from("Windows Search"),
        "runtimebroker.exe" => return String::from("Runtime Broker"),
        "sihost.exe" => return String::from("Shell Infrastructure Host"),
        "taskhostw.exe" => return String::from("Task Host Window"),
        "shellexperiencehost.exe" => return String::from("Shell Experience Host"),
        "startmenuexperiencehost.exe" => return String::from("Start Menu Host"),
        "textinputhost.exe" => return String::from("Text Input Host"),
        "applicationframehost.exe" => return String::from("Application Frame Host"),
        "audiodg.exe" => return String::from("Windows Audio Graph"),
        "spoolsv.exe" => return String::from("Print Spooler"),
        "ctfmon.exe" => return String::from("CTF Loader"),
        "conhost.exe" => return String::from("Console Window Host"),
        "dllhost.exe" => return String::from("COM Surrogate"),
        "otto-os.exe" => return String::from("Otto OS"),
        "msedge.exe" => return String::from("Microsoft Edge"),
        "chrome.exe" => return String::from("Google Chrome"),
        "firefox.exe" => return String::from("Mozilla Firefox"),
        "brave.exe" => return String::from("Brave Browser"),
        "code.exe" => return String::from("Visual Studio Code"),
        "windowsterminal.exe" => return String::from("Windows Terminal"),
        _ => {}
    }
    let stem = exe_name_lower
        .strip_suffix(".exe")
        .unwrap_or(exe_name_lower);
    let mut out = String::with_capacity(stem.len());
    let mut capitalize = true;
    for ch in stem.chars() {
        if ch == '-' || ch == '_' || ch == '.' || ch == ' ' {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            capitalize = true;
        } else if capitalize {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
            capitalize = false;
        } else {
            out.push(ch);
        }
    }
    if out.is_empty() {
        String::from(exe_name_lower)
    } else {
        out
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GroupProtectionScope {
    AllProtected,
    HasUnprotected,
}

#[derive(Clone, Copy)]
struct ActionAllowance {
    controllable: bool,
    terminatable: bool,
}

struct GroupAccumulator {
    exe_path: Option<String>,
    process_count: usize,
    primary_pid: u32,
    primary_memory: u64,
    memory_bytes: u64,
    cpu_tenths_percent: u32,
    any_foreground: bool,
    has_visible_window: bool,
    allowance: ActionAllowance,
    protection_scope: GroupProtectionScope,
    strongest_protection: Option<ProtectionReason>,
    throttle_kind: ProcessThrottleKind,
}

fn merge_protection(
    current: Option<ProtectionReason>,
    candidate: Option<ProtectionReason>,
) -> Option<ProtectionReason> {
    match (current, candidate) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    }
}

fn merge_throttle(
    current: ProcessThrottleKind,
    candidate: ProcessThrottleKind,
) -> ProcessThrottleKind {
    match (current, candidate) {
        (ProcessThrottleKind::ManualEcoQos, _) | (_, ProcessThrottleKind::ManualEcoQos) => {
            ProcessThrottleKind::ManualEcoQos
        }
        (ProcessThrottleKind::AutoEcoQos, _) | (_, ProcessThrottleKind::AutoEcoQos) => {
            ProcessThrottleKind::AutoEcoQos
        }
        (ProcessThrottleKind::None, ProcessThrottleKind::None) => ProcessThrottleKind::None,
    }
}

fn accumulate_audit_sample(
    grouped: &mut HashMap<String, GroupAccumulator>,
    sample: &RawProcessAuditSample,
    exe_lower: String,
) {
    let protection = classify_process_protection(
        &exe_lower,
        sample.pid,
        sample.session_id,
        sample.is_critical,
        sample.controllable,
    );
    let path = sample
        .exe_path
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    let sample_scope = if protection.is_some() {
        GroupProtectionScope::AllProtected
    } else {
        GroupProtectionScope::HasUnprotected
    };

    grouped
        .entry(exe_lower)
        .and_modify(|acc| {
            acc.process_count += 1;
            if acc.exe_path.is_none() {
                acc.exe_path.clone_from(&path);
            }
            if sample.memory_bytes > acc.primary_memory {
                acc.primary_memory = sample.memory_bytes;
                acc.primary_pid = sample.pid;
            }
            acc.memory_bytes = acc.memory_bytes.saturating_add(sample.memory_bytes);
            acc.cpu_tenths_percent = acc
                .cpu_tenths_percent
                .saturating_add(u32::from(sample.cpu_tenths_percent));
            acc.any_foreground |= sample.is_foreground;
            acc.has_visible_window |= sample.has_visible_window;
            acc.allowance.terminatable |= sample.terminatable;
            acc.allowance.controllable |= sample.controllable;
            if protection.is_none() {
                acc.protection_scope = GroupProtectionScope::HasUnprotected;
            }
            if matches!(
                protection,
                Some(
                    ProtectionReason::OttoSelf
                        | ProtectionReason::CriticalProcess
                        | ProtectionReason::DesktopShell
                )
            ) || sample.is_critical
            {
                acc.protection_scope = GroupProtectionScope::AllProtected;
            }
            acc.strongest_protection = merge_protection(acc.strongest_protection, protection);
            acc.throttle_kind = merge_throttle(acc.throttle_kind, sample.throttle_kind);
        })
        .or_insert_with(|| GroupAccumulator {
            exe_path: path,
            process_count: 1,
            primary_pid: sample.pid,
            primary_memory: sample.memory_bytes,
            memory_bytes: sample.memory_bytes,
            cpu_tenths_percent: u32::from(sample.cpu_tenths_percent),
            any_foreground: sample.is_foreground,
            has_visible_window: sample.has_visible_window,
            allowance: ActionAllowance {
                controllable: sample.controllable,
                terminatable: sample.terminatable,
            },
            protection_scope: sample_scope,
            strongest_protection: protection,
            throttle_kind: sample.throttle_kind,
        });
}

fn finalize_audit_group(exe_name: String, acc: GroupAccumulator) -> AuditedAppGroup {
    let named_protection = classify_process_protection(&exe_name, 100, 1, false, true);
    let protection = if let Some(reason) = named_protection {
        Some(reason)
    } else if acc.protection_scope == GroupProtectionScope::AllProtected
        || !acc.allowance.controllable
    {
        acc.strongest_protection
    } else {
        None
    };
    let safety = if protection.is_some() {
        ProcessSafety::SystemProtected
    } else if acc.any_foreground {
        ProcessSafety::ActiveWindow
    } else {
        ProcessSafety::SafeBackground
    };
    let can_throttle = safety != ProcessSafety::SystemProtected;
    let can_terminate = safety == ProcessSafety::SafeBackground && acc.allowance.terminatable;
    let display_name = friendly_process_display_name(&exe_name);
    AuditedAppGroup {
        exe_name,
        display_name,
        exe_path: acc.exe_path,
        process_count: acc.process_count,
        primary_pid: acc.primary_pid,
        memory_bytes: acc.memory_bytes,
        cpu_tenths_percent: acc.cpu_tenths_percent,
        safety,
        has_visible_window: acc.has_visible_window,
        protection,
        throttle_kind: acc.throttle_kind,
        can_throttle,
        can_terminate,
    }
}

#[must_use]
pub fn summarize_process_audit(samples: &[RawProcessAuditSample]) -> ProcessAuditReport {
    let mut grouped: HashMap<String, GroupAccumulator> = HashMap::new();
    let mut total_processes = 0usize;

    for sample in samples {
        let exe_lower = sample.exe_name.trim().to_ascii_lowercase();
        if sample.pid == 0
            || exe_lower.is_empty()
            || exe_lower == "[system process]"
            || exe_lower == "system idle process"
        {
            continue;
        }
        total_processes += 1;
        accumulate_audit_sample(&mut grouped, sample, exe_lower);
    }

    let mut groups: Vec<AuditedAppGroup> = grouped
        .into_iter()
        .map(|(exe_name, acc)| finalize_audit_group(exe_name, acc))
        .collect();

    groups.sort_by(|a, b| {
        b.memory_bytes
            .cmp(&a.memory_bytes)
            .then_with(|| b.cpu_tenths_percent.cmp(&a.cpu_tenths_percent))
            .then_with(|| a.exe_name.cmp(&b.exe_name))
    });

    let mut report = ProcessAuditReport {
        total_processes,
        ..ProcessAuditReport::default()
    };
    for group in &groups {
        if group.safety == ProcessSafety::SafeBackground {
            report.background_group_count += 1;
            report.background_memory_bytes = report
                .background_memory_bytes
                .saturating_add(group.memory_bytes);
            if group.is_memory_buffer() {
                report.memory_buffer_count += 1;
            }
            if group.is_safe_to_end_recommended() {
                report.safe_to_end_count += 1;
            }
        } else if group.safety == ProcessSafety::SystemProtected {
            report.protected_group_count += 1;
        }
        match group.throttle_kind {
            ProcessThrottleKind::AutoEcoQos => report.auto_throttled_count += 1,
            ProcessThrottleKind::ManualEcoQos => report.manual_throttled_count += 1,
            ProcessThrottleKind::None => {}
        }
    }
    report.groups = groups;
    report
}

#[must_use]
pub fn format_memory_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if bytes >= GIB {
        let tenths = bytes.saturating_mul(10) / GIB;
        format!("{}.{} GB", tenths / 10, tenths % 10)
    } else if bytes >= MIB {
        format!("{} MB", bytes.saturating_add(MIB / 2) / MIB)
    } else if bytes >= KIB {
        format!("{} KB", bytes.saturating_add(KIB / 2) / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[must_use]
pub fn format_cpu_tenths(tenths: u32) -> String {
    if tenths == 0 {
        String::from("0% CPU")
    } else {
        format!("{}.{}% CPU", tenths / 10, tenths % 10)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSample {
    pub pid: u32,
    pub name: String,
    pub cpu_100ns: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct HogDecision {
    pub throttle: Vec<u32>,
    pub restore: Vec<u32>,
}

#[derive(Debug, Default)]
pub struct HogTracker {
    previous_cpu: HashMap<u32, u64>,
    streak: HashMap<u32, u8>,
    throttled: HashSet<u32>,
}

impl HogTracker {
    pub fn update(
        &mut self,
        samples: &[ProcessSample],
        elapsed_100ns: u64,
        foreground_name: Option<&str>,
    ) -> HogDecision {
        let mut decision = HogDecision::default();
        let mut previous_cpu = HashMap::with_capacity(samples.len());
        let mut streak = HashMap::new();
        for sample in samples {
            previous_cpu.insert(sample.pid, sample.cpu_100ns);
            let in_foreground = foreground_name.is_some_and(|name| name == sample.name);
            if self.throttled.contains(&sample.pid) {
                if in_foreground {
                    self.throttled.remove(&sample.pid);
                    decision.restore.push(sample.pid);
                }
                continue;
            }
            let busy = self.previous_cpu.get(&sample.pid).is_some_and(|&before| {
                sample.cpu_100ns.saturating_sub(before) * 100
                    >= elapsed_100ns * HOG_CPU_PERCENT_OF_CORE
            }) && elapsed_100ns > 0;
            if !busy || in_foreground || NEVER_THROTTLE.contains(&sample.name.as_str()) {
                continue;
            }
            let count = self.streak.get(&sample.pid).copied().unwrap_or(0) + 1;
            if count >= HOG_SUSTAINED_SAMPLES {
                self.throttled.insert(sample.pid);
                decision.throttle.push(sample.pid);
            } else {
                streak.insert(sample.pid, count);
            }
        }
        self.throttled.retain(|pid| previous_cpu.contains_key(pid));
        self.previous_cpu = previous_cpu;
        self.streak = streak;
        decision
    }

    pub fn release_all(&mut self) -> Vec<u32> {
        self.previous_cpu.clear();
        self.streak.clear();
        self.throttled.drain().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVAL: u64 = HOG_SAMPLE_INTERVAL_SECS * 10_000_000;
    const BUSY_STEP: u64 = INTERVAL / 2;

    fn sample(pid: u32, name: &str, cpu: u64) -> ProcessSample {
        ProcessSample {
            pid,
            name: name.to_string(),
            cpu_100ns: cpu,
        }
    }

    fn run(
        tracker: &mut HogTracker,
        rounds: u64,
        name: &str,
        foreground: Option<&str>,
    ) -> HogDecision {
        let mut last = HogDecision::default();
        for round in 0..=rounds {
            last = tracker.update(&[sample(7, name, round * BUSY_STEP)], INTERVAL, foreground);
        }
        last
    }

    #[test]
    fn sustained_background_load_is_throttled_once() {
        let mut tracker = HogTracker::default();
        let decision = run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            Some("code.exe"),
        );
        assert_eq!(decision.throttle, vec![7]);
        let again = tracker.update(
            &[sample(7, "sync.exe", 99 * BUSY_STEP)],
            INTERVAL,
            Some("code.exe"),
        );
        assert_eq!(again, HogDecision::default());
    }

    #[test]
    fn short_spike_is_ignored() {
        let mut tracker = HogTracker::default();
        let decision = run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES) - 1,
            "sync.exe",
            None,
        );
        assert!(decision.throttle.is_empty());
    }

    #[test]
    fn foreground_family_and_protected_names_are_exempt() {
        let mut tracker = HogTracker::default();
        let rounds = u64::from(HOG_SUSTAINED_SAMPLES) + 2;
        assert!(
            run(&mut tracker, rounds, "chrome.exe", Some("chrome.exe"))
                .throttle
                .is_empty()
        );
        let mut tracker = HogTracker::default();
        assert!(
            run(&mut tracker, rounds, "explorer.exe", None)
                .throttle
                .is_empty()
        );
    }

    #[test]
    fn idle_process_is_never_throttled() {
        let mut tracker = HogTracker::default();
        for _ in 0..10 {
            let decision = tracker.update(&[sample(7, "idle.exe", 5)], INTERVAL, None);
            assert!(decision.throttle.is_empty());
        }
    }

    #[test]
    fn throttled_process_is_restored_when_it_becomes_foreground() {
        let mut tracker = HogTracker::default();
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        let decision = tracker.update(
            &[sample(7, "sync.exe", 99 * BUSY_STEP)],
            INTERVAL,
            Some("sync.exe"),
        );
        assert_eq!(decision.restore, vec![7]);
    }

    #[test]
    fn release_all_returns_live_throttled_pids_and_exited_ones_are_dropped() {
        let mut tracker = HogTracker::default();
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        assert_eq!(tracker.release_all(), vec![7]);
        run(
            &mut tracker,
            u64::from(HOG_SUSTAINED_SAMPLES),
            "sync.exe",
            None,
        );
        tracker.update(&[], INTERVAL, None);
        assert!(tracker.release_all().is_empty());
    }

    fn audit_sample(
        pid: u32,
        exe_name: &str,
        memory_mb: u64,
        cpu_tenths: u16,
        session_id: u32,
        controllable: bool,
    ) -> RawProcessAuditSample {
        RawProcessAuditSample {
            pid,
            exe_name: String::from(exe_name),
            exe_path: Some(format!(r"C:\Apps\{exe_name}")),
            memory_bytes: memory_mb * 1024 * 1024,
            cpu_tenths_percent: cpu_tenths,
            session_id,
            is_critical: false,
            controllable,
            terminatable: controllable,
            is_foreground: false,
            has_visible_window: false,
            throttle_kind: ProcessThrottleKind::None,
        }
    }

    #[test]
    fn classifies_system_shell_otto_and_critical_processes_as_protected() {
        assert_eq!(
            classify_process_protection("otto-os.exe", 500, 1, true, true),
            Some(ProtectionReason::OttoSelf)
        );
        assert_eq!(
            classify_process_protection("csrss.exe", 400, 1, false, false),
            Some(ProtectionReason::CriticalProcess)
        );
        assert_eq!(
            classify_process_protection("anyapp.exe", 4, 0, false, false),
            Some(ProtectionReason::CriticalProcess)
        );
        assert_eq!(
            classify_process_protection("custom.exe", 500, 1, true, true),
            Some(ProtectionReason::CriticalProcess)
        );
        assert_eq!(
            classify_process_protection("explorer.exe", 500, 1, false, true),
            Some(ProtectionReason::DesktopShell)
        );
        assert_eq!(
            classify_process_protection("dwm.exe", 500, 1, false, true),
            Some(ProtectionReason::DesktopShell)
        );
        assert_eq!(
            classify_process_protection("svchost.exe", 500, 0, false, false),
            Some(ProtectionReason::WindowsKernelOrService)
        );
        assert_eq!(
            classify_process_protection("myservice.exe", 500, 0, false, true),
            Some(ProtectionReason::WindowsKernelOrService)
        );
        assert_eq!(
            classify_process_protection("admin_tool.exe", 500, 1, false, false),
            Some(ProtectionReason::ElevatedOrSystemToken)
        );
        assert_eq!(
            classify_process_protection("discord.exe", 500, 1, false, true),
            None
        );
    }

    #[test]
    fn summarize_process_audit_groups_by_exe_and_detects_memory_buffers() {
        let mut fg = audit_sample(10, "Code.exe", 300, 20, 1, true);
        fg.is_foreground = true;
        fg.has_visible_window = true;
        let mut hidden_buf_1 = audit_sample(20, "Discord.exe", 90, 5, 1, true);
        hidden_buf_1.throttle_kind = ProcessThrottleKind::AutoEcoQos;
        let hidden_buf_2 = audit_sample(21, "discord.exe", 70, 3, 1, true);
        let svc = audit_sample(30, "svchost.exe", 200, 1, 0, false);
        let idle_placeholder = audit_sample(0, "System Idle Process", 0, 900, 0, false);

        let report =
            summarize_process_audit(&[fg, hidden_buf_1, hidden_buf_2, svc, idle_placeholder]);

        assert_eq!(report.total_processes, 4);
        assert_eq!(report.background_group_count, 1);
        assert_eq!(report.memory_buffer_count, 1);
        assert_eq!(report.auto_throttled_count, 1);
        assert_eq!(report.protected_group_count, 1);
        assert_eq!(report.background_memory_bytes, 160 * 1024 * 1024);

        let discord = report
            .groups
            .iter()
            .find(|g| g.exe_name == "discord.exe")
            .expect("discord group");
        assert_eq!(discord.process_count, 2);
        assert_eq!(discord.primary_pid, 20);
        assert_eq!(discord.cpu_tenths_percent, 8);
        assert_eq!(discord.safety, ProcessSafety::SafeBackground);
        assert!(discord.is_memory_buffer());
        assert!(discord.can_throttle);
        assert!(discord.can_terminate);

        let code = report
            .groups
            .iter()
            .find(|g| g.exe_name == "code.exe")
            .expect("code group");
        assert_eq!(code.safety, ProcessSafety::ActiveWindow);
        assert!(code.can_throttle);
        assert!(!code.can_terminate);

        let svchost = report
            .groups
            .iter()
            .find(|g| g.exe_name == "svchost.exe")
            .expect("svchost group");
        assert_eq!(svchost.safety, ProcessSafety::SystemProtected);
        assert!(!svchost.can_throttle);
        assert!(!svchost.can_terminate);
        assert_eq!(report.safe_to_end_count, 1);
        assert!(discord.is_safe_to_end_recommended());
    }

    #[test]
    fn safe_to_end_requires_heavy_headless_non_utility_background() {
        let heavy_hidden = audit_sample(10, "discord.exe", 160, 0, 1, true);
        let light_hidden = audit_sample(11, "helper.exe", 120, 0, 1, true);
        let mut heavy_visible = audit_sample(12, "slack.exe", 220, 0, 1, true);
        heavy_visible.has_visible_window = true;
        let onedrive = audit_sample(13, "OneDrive.exe", 260, 1, 1, true);
        let tailscale = audit_sample(14, "tailscaled.exe", 200, 1, 1, true);

        let report = summarize_process_audit(&[
            heavy_hidden,
            light_hidden,
            heavy_visible,
            onedrive,
            tailscale,
        ]);

        assert_eq!(report.safe_to_end_count, 1);
        let check = |name: &str| {
            report
                .groups
                .iter()
                .find(|g| g.exe_name == name)
                .expect(name)
                .is_safe_to_end_recommended()
        };
        assert!(check("discord.exe"));
        assert!(!check("helper.exe"));
        assert!(!check("slack.exe"));
        assert!(!check("onedrive.exe"));
        assert!(!check("tailscaled.exe"));
    }

    #[test]
    fn formats_memory_and_cpu_cleanly() {
        assert_eq!(format_memory_bytes(512), "512 B");
        assert_eq!(format_memory_bytes(2048), "2 KB");
        assert_eq!(format_memory_bytes(150 * 1024 * 1024), "150 MB");
        assert_eq!(format_memory_bytes(2_500_000_000), "2.3 GB");
        assert_eq!(format_cpu_tenths(0), "0% CPU");
        assert_eq!(format_cpu_tenths(125), "12.5% CPU");
    }
}
