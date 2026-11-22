use std::cell::RefCell;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime};

use otto_core::{PixelLimits, RgbBuffer, bgra_layout};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{BitBlt, CAPTUREBLT, ROP_CODE, SRCCOPY};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory,
    WICBitmapDitherTypeNone, WICBitmapInterpolationModeLinear, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand, WICRect,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::WindowsAndMessaging::{
    IsWindowVisible, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    SetWindowDisplayAffinity, SystemParametersInfoW, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
};

use crate::com::ComGuard;
use crate::gdi::{CaptureCanvas, ScreenDc};
use crate::screen::primary_screen;
use crate::worker::join_within;

const STOP_JOIN_LIMIT: Duration = Duration::from_millis(1500);
const MAX_WALLPAPER_FILE_BYTES: u64 = 64 * 1024 * 1024;
const WALLPAPER_CHECK_TTL: Duration = Duration::from_secs(5);

#[must_use]
pub fn is_surface_visible(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe { IsWindowVisible(HWND(hwnd as *mut c_void)).as_bool() }
}

#[must_use]
pub fn exclude_from_capture(hwnd: isize) -> bool {
    set_affinity(hwnd, true)
}

#[must_use]
pub fn include_in_capture(hwnd: isize) -> bool {
    set_affinity(hwnd, false)
}

fn set_affinity(hwnd: isize, excluded: bool) -> bool {
    let affinity = if excluded {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    unsafe { SetWindowDisplayAffinity(HWND(hwnd as *mut c_void), affinity) }.is_ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackdropRequest {
    pub id: u64,
    pub origin: (i32, i32),
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BackdropFrame {
    pub id: u64,
    pub origin: (i32, i32),
    pub signature: u64,
    pub pixels: RgbBuffer,
}

const SIGNATURE_SEED: u64 = 0xcbf2_9ce4_8422_2325;
const SIGNATURE_PRIME: u64 = 0x0000_0100_0000_01b3;

fn pixel_signature(bytes: &[u8]) -> u64 {
    let mut chunks = bytes.chunks_exact(8);
    let mut hash = SIGNATURE_SEED ^ bytes.len() as u64;
    for chunk in chunks.by_ref() {
        let mut word = [0u8; 8];
        word.copy_from_slice(chunk);
        hash = (hash ^ u64::from_le_bytes(word)).wrapping_mul(SIGNATURE_PRIME);
        hash ^= hash >> 29;
    }
    for byte in chunks.remainder() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(SIGNATURE_PRIME);
    }
    hash
}

struct BackdropGrabber {
    screen: ScreenDc,
    canvas: CaptureCanvas,
}

impl BackdropGrabber {
    fn new(width: u32, height: u32) -> Option<Self> {
        let layout = bgra_layout(width, height, PixelLimits::BACKDROP).ok()?;
        let screen = ScreenDc::acquire()?;
        let canvas = CaptureCanvas::create(&screen, layout)?;
        Some(Self { screen, canvas })
    }

    fn matches(&self, request: &BackdropRequest) -> bool {
        self.matches_size(request.width, request.height)
    }

    fn matches_size(&self, width: u32, height: u32) -> bool {
        let layout = self.canvas.layout();
        layout.width == width && layout.height == height
    }

    fn grab(&self, request: &BackdropRequest) -> Option<BackdropFrame> {
        let (signature, pixels) = self.grab_with_signature(request.origin, SRCCOPY | CAPTUREBLT)?;
        Some(BackdropFrame {
            id: request.id,
            origin: request.origin,
            signature,
            pixels,
        })
    }

    fn grab_fast(&self, origin: (i32, i32)) -> Option<RgbBuffer> {
        self.blit(origin, SRCCOPY)?;
        let layout = self.canvas.layout();
        RgbBuffer::from_bgra(layout.width, layout.height, self.canvas.as_bytes()).ok()
    }

    fn grab_with_signature(&self, origin: (i32, i32), rop: ROP_CODE) -> Option<(u64, RgbBuffer)> {
        self.blit(origin, rop)?;
        let layout = self.canvas.layout();
        let bytes = self.canvas.as_bytes();
        let signature = pixel_signature(bytes);
        let pixels = RgbBuffer::from_bgra(layout.width, layout.height, bytes).ok()?;
        Some((signature, pixels))
    }

    fn blit(&self, origin: (i32, i32), rop: ROP_CODE) -> Option<()> {
        let layout = self.canvas.layout();
        unsafe {
            BitBlt(
                self.canvas.dc(),
                0,
                0,
                i32::try_from(layout.width).ok()?,
                i32::try_from(layout.height).ok()?,
                Some(self.screen.handle()),
                origin.0,
                origin.1,
                rop,
            )
        }
        .ok()?;
        Some(())
    }
}

struct CachedWallpaper {
    path: PathBuf,
    modified: Option<SystemTime>,
    last_checked: Instant,
    width: u32,
    height: u32,
    bgra: Vec<u8>,
}

thread_local! {
    static CAPTURE_GRABBER: RefCell<Option<BackdropGrabber>> = const { RefCell::new(None) };
    static WALLPAPER_CACHE: RefCell<Option<CachedWallpaper>> = const { RefCell::new(None) };
}

pub fn release_backdrop_capture_cache() {
    let _ = CAPTURE_GRABBER.try_with(|slot| {
        if let Ok(mut grabber) = slot.try_borrow_mut() {
            *grabber = None;
        }
    });
    let _ = WALLPAPER_CACHE.try_with(|slot| {
        if let Ok(mut cache) = slot.try_borrow_mut() {
            *cache = None;
        }
    });
}

fn active_wallpaper_path() -> Option<PathBuf> {
    let mut buffer = [0u16; 520];
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr().cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .is_ok();
    if ok {
        let end = buffer
            .iter()
            .position(|&ch| ch == 0)
            .unwrap_or(buffer.len());
        if end > 0 {
            let candidate = PathBuf::from(String::from_utf16_lossy(&buffer[..end]));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let appdata = std::env::var_os("APPDATA")?;
    let transcoded = PathBuf::from(appdata).join("Microsoft\\Windows\\Themes\\TranscodedWallpaper");
    transcoded.is_file().then_some(transcoded)
}

fn decode_wallpaper_bgra(path: &PathBuf, target_w: u32, target_h: u32) -> Option<Vec<u8>> {
    let layout = bgra_layout(target_w, target_h, PixelLimits::WINDOW_SNAPSHOT).ok()?;
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() == 0 || meta.len() > MAX_WALLPAPER_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let _com = ComGuard::single_threaded();
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let stream = factory.CreateStream().ok()?;
        stream.InitializeFromMemory(&bytes).ok()?;
        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .ok()?;
        let frame = decoder.GetFrame(0).ok()?;
        let mut src_w = 0u32;
        let mut src_h = 0u32;
        frame.GetSize(&mut src_w, &mut src_h).ok()?;
        if src_w == 0 || src_h == 0 {
            return None;
        }
        let scale_x = f64::from(target_w) / f64::from(src_w);
        let scale_y = f64::from(target_h) / f64::from(src_h);
        let scale = scale_x.max(scale_y);
        let scaled_w = ((f64::from(src_w) * scale).round() as u32).max(target_w);
        let scaled_h = ((f64::from(src_h) * scale).round() as u32).max(target_h);
        let scaler = factory.CreateBitmapScaler().ok()?;
        scaler
            .Initialize(&frame, scaled_w, scaled_h, WICBitmapInterpolationModeLinear)
            .ok()?;
        let converter = factory.CreateFormatConverter().ok()?;
        converter
            .Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .ok()?;
        let crop_x = i32::try_from((scaled_w - target_w) / 2).ok()?;
        let crop_y = i32::try_from((scaled_h - target_h) / 2).ok()?;
        let rect = WICRect {
            X: crop_x,
            Y: crop_y,
            Width: i32::try_from(target_w).ok()?,
            Height: i32::try_from(target_h).ok()?,
        };
        let mut out = vec![0u8; layout.byte_len];
        converter
            .CopyPixels(&raw const rect, layout.stride, &mut out)
            .ok()?;
        Some(out)
    }
}

fn crop_bgra_region(
    source: &[u8],
    source_w: u32,
    source_h: u32,
    origin: (i32, i32),
    width: u32,
    height: u32,
) -> Option<RgbBuffer> {
    let layout = bgra_layout(width, height, PixelLimits::BACKDROP).ok()?;
    if source_w == 0 || source_h == 0 {
        return None;
    }
    let expected_source_len = (source_w as usize)
        .checked_mul(source_h as usize)?
        .checked_mul(4)?;
    if source.len() < expected_source_len {
        return None;
    }
    let max_x = i32::try_from(source_w).ok()?.saturating_sub(1).max(0);
    let max_y = i32::try_from(source_h).ok()?.saturating_sub(1).max(0);
    let mut cropped = vec![0u8; layout.byte_len];
    let src_stride = source_w as usize * 4;
    let dst_stride = layout.stride as usize;
    for row in 0..height {
        let sy = (origin.1 + row as i32).clamp(0, max_y) as usize;
        let src_row = &source[sy * src_stride..(sy + 1) * src_stride];
        let dst_row = &mut cropped[row as usize * dst_stride..(row as usize + 1) * dst_stride];
        if origin.0 >= 0 && origin.0.saturating_add(width as i32) <= max_x + 1 {
            let start = origin.0 as usize * 4;
            dst_row.copy_from_slice(&src_row[start..start + dst_stride]);
        } else {
            for col in 0..width {
                let sx = (origin.0 + col as i32).clamp(0, max_x) as usize * 4;
                let dx = col as usize * 4;
                dst_row[dx..dx + 4].copy_from_slice(&src_row[sx..sx + 4]);
            }
        }
    }
    RgbBuffer::from_bgra(width, height, &cropped).ok()
}

#[must_use]
pub fn capture_wallpaper_now(origin: (i32, i32), width: u32, height: u32) -> Option<RgbBuffer> {
    let _ = bgra_layout(width, height, PixelLimits::BACKDROP).ok()?;
    let screen = primary_screen();
    let screen_w = u32::try_from(screen.width.max(1)).ok()?;
    let screen_h = u32::try_from(screen.height.max(1)).ok()?;
    WALLPAPER_CACHE.with(|slot| {
        let mut cache = slot.try_borrow_mut().ok()?;
        let now = Instant::now();
        let needs_check = cache.as_ref().is_none_or(|held| {
            held.width != screen_w
                || held.height != screen_h
                || now.saturating_duration_since(held.last_checked) >= WALLPAPER_CHECK_TTL
        });
        if needs_check {
            if let Some(held) = cache.as_mut()
                && held.width == screen_w
                && held.height == screen_h
            {
                held.last_checked = now;
                let path = active_wallpaper_path()?;
                let modified = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                if held.path != path || held.modified != modified {
                    let bgra = decode_wallpaper_bgra(&path, screen_w, screen_h)?;
                    *held = CachedWallpaper {
                        path,
                        modified,
                        last_checked: now,
                        width: screen_w,
                        height: screen_h,
                        bgra,
                    };
                }
            } else {
                let path = active_wallpaper_path()?;
                let modified = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok());
                let bgra = decode_wallpaper_bgra(&path, screen_w, screen_h)?;
                *cache = Some(CachedWallpaper {
                    path,
                    modified,
                    last_checked: now,
                    width: screen_w,
                    height: screen_h,
                    bgra,
                });
            }
        }
        let held = cache.as_ref()?;
        crop_bgra_region(&held.bgra, held.width, held.height, origin, width, height)
    })
}

#[must_use]
pub fn capture_backdrop_now(origin: (i32, i32), width: u32, height: u32) -> Option<RgbBuffer> {
    CAPTURE_GRABBER.with(|slot| {
        let mut grabber = slot.try_borrow_mut().ok()?;
        if !grabber
            .as_ref()
            .is_some_and(|held| held.matches_size(width, height))
        {
            *grabber = BackdropGrabber::new(width, height);
        }
        grabber.as_ref()?.grab_fast(origin)
    })
}

fn screen_grab() -> impl FnMut(&BackdropRequest) -> Option<BackdropFrame> {
    let mut grabber: Option<BackdropGrabber> = None;
    move |request| {
        if !grabber.as_ref().is_some_and(|held| held.matches(request)) {
            grabber = BackdropGrabber::new(request.width, request.height);
        }
        grabber.as_ref()?.grab(request)
    }
}

#[derive(Default)]
struct Mailbox {
    request: Option<BackdropRequest>,
    frame: Option<BackdropFrame>,
    capturing: bool,
    stopping: bool,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Mailbox> {
        self.mailbox.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn next_request(&self) -> Option<BackdropRequest> {
        let mut mailbox = self.lock();
        loop {
            if mailbox.stopping {
                return None;
            }
            if let Some(request) = mailbox.request.take() {
                mailbox.capturing = true;
                return Some(request);
            }
            mailbox = self
                .wake
                .wait(mailbox)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }
}

pub struct BackdropWorker {
    shared: Arc<Shared>,
    handle: Option<JoinHandle<()>>,
}

impl BackdropWorker {
    #[must_use]
    pub fn spawn(notify: impl Fn() + Send + 'static) -> Self {
        Self::spawn_with(screen_grab, notify)
    }

    fn spawn_with<G>(
        make_grab: impl FnOnce() -> G + Send + 'static,
        notify: impl Fn() + Send + 'static,
    ) -> Self
    where
        G: FnMut(&BackdropRequest) -> Option<BackdropFrame>,
    {
        let shared = Arc::new(Shared {
            mailbox: Mutex::new(Mailbox::default()),
            wake: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::spawn(move || {
            let mut grab = make_grab();
            while let Some(request) = worker_shared.next_request() {
                let frame = grab(&request);
                let mut mailbox = worker_shared.lock();
                if let Some(frame) = frame {
                    mailbox.frame = Some(frame);
                }
                mailbox.capturing = mailbox.request.is_some();
                drop(mailbox);
                notify();
            }
        });
        Self {
            shared,
            handle: Some(handle),
        }
    }

    pub fn request(&self, request: BackdropRequest) {
        self.shared.lock().request = Some(request);
        self.shared.wake.notify_one();
    }

    #[must_use]
    pub fn take_frame(&self) -> Option<BackdropFrame> {
        self.shared.lock().frame.take()
    }

    #[must_use]
    pub fn is_capturing(&self) -> bool {
        let mailbox = self.shared.lock();
        mailbox.capturing || mailbox.request.is_some()
    }
}

impl Drop for BackdropWorker {
    fn drop(&mut self) {
        self.shared.lock().stopping = true;
        self.shared.wake.notify_all();
        if let Some(handle) = self.handle.take() {
            let _ = join_within(handle, STOP_JOIN_LIMIT);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc::{Receiver, channel};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use otto_core::RgbBuffer;

    use super::{
        BackdropFrame, BackdropGrabber, BackdropRequest, BackdropWorker, capture_backdrop_now,
        capture_wallpaper_now, crop_bgra_region, is_surface_visible, pixel_signature,
    };

    #[test]
    fn the_signature_tracks_every_byte_including_the_unaligned_tail() {
        let base = vec![7u8; 4 * 13];
        assert_eq!(pixel_signature(&base), pixel_signature(&base.clone()));
        for index in [0, 8, 31, base.len() - 1] {
            let mut changed = base.clone();
            changed[index] ^= 1;
            assert_ne!(
                pixel_signature(&base),
                pixel_signature(&changed),
                "byte {index}"
            );
        }
        assert_ne!(
            pixel_signature(&base),
            pixel_signature(&base[..base.len() - 1])
        );
    }

    const WAIT: Duration = Duration::from_secs(3);

    fn request(id: u64) -> BackdropRequest {
        BackdropRequest {
            id,
            origin: (10, 20),
            width: 8,
            height: 4,
        }
    }

    fn frame_for(request: &BackdropRequest) -> BackdropFrame {
        BackdropFrame {
            id: request.id,
            origin: request.origin,
            signature: request.id,
            pixels: RgbBuffer::filled(request.width as usize, request.height as usize, [0.5; 3]),
        }
    }

    fn wait_until(mut ready: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < WAIT {
            if ready() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        false
    }

    fn wait_for_frame(worker: &BackdropWorker, id: u64) -> Option<BackdropFrame> {
        let mut found = None;
        wait_until(|| {
            found = worker.take_frame().filter(|frame| frame.id == id);
            found.is_some()
        });
        found
    }

    #[test]
    fn a_request_becomes_a_frame_and_notifies_once() {
        let notified = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&notified);
        let worker = BackdropWorker::spawn_with(
            || |request: &BackdropRequest| Some(frame_for(request)),
            move || {
                counter.fetch_add(1, Ordering::SeqCst);
            },
        );
        worker.request(request(1));
        let frame = wait_for_frame(&worker, 1).expect("the frame should arrive");
        assert_eq!(frame.origin, (10, 20));
        assert_eq!(frame.pixels.width(), 8);
        assert_eq!(notified.load(Ordering::SeqCst), 1);
        assert!(worker.take_frame().is_none());
    }

    #[test]
    fn requests_made_while_busy_collapse_to_the_newest() {
        let (release, gate): (_, Receiver<()>) = channel();
        let grabbed = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&grabbed);
        let gate = Mutex::new(gate);
        let worker = BackdropWorker::spawn_with(
            move || {
                move |request: &BackdropRequest| {
                    record.lock().unwrap().push(request.id);
                    if request.id == 1 {
                        let _ = gate.lock().unwrap().recv_timeout(WAIT);
                    }
                    Some(frame_for(request))
                }
            },
            || {},
        );
        worker.request(request(1));
        assert!(wait_until(|| grabbed.lock().unwrap().contains(&1)));
        worker.request(request(2));
        worker.request(request(3));
        worker.request(request(4));
        release.send(()).unwrap();
        assert!(wait_for_frame(&worker, 4).is_some());
        assert_eq!(*grabbed.lock().unwrap(), vec![1, 4]);
    }

    #[test]
    fn a_failed_grab_publishes_nothing_and_the_worker_keeps_serving() {
        let worker = BackdropWorker::spawn_with(
            || {
                |request: &BackdropRequest| {
                    (request.id.is_multiple_of(2)).then(|| frame_for(request))
                }
            },
            || {},
        );
        worker.request(request(1));
        std::thread::sleep(Duration::from_millis(60));
        assert!(worker.take_frame().is_none());
        worker.request(request(2));
        assert!(wait_for_frame(&worker, 2).is_some());
    }

    #[test]
    fn dropping_the_worker_stops_an_idle_thread_promptly() {
        let worker = BackdropWorker::spawn_with(
            || |request: &BackdropRequest| Some(frame_for(request)),
            || {},
        );
        let start = Instant::now();
        drop(worker);
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn the_screen_grabber_returns_a_frame_of_the_requested_size() {
        let request = BackdropRequest {
            id: 7,
            origin: (0, 0),
            width: 64,
            height: 32,
        };
        let grabber = BackdropGrabber::new(request.width, request.height)
            .expect("a grabber should be created on the desktop");
        assert!(grabber.matches(&request));
        assert!(!grabber.matches(&BackdropRequest {
            width: 65,
            ..request
        }));
        let frame = grabber
            .grab(&request)
            .expect("the screen should be readable");
        assert_eq!(frame.id, 7);
        assert_eq!((frame.pixels.width(), frame.pixels.height()), (64, 32));
    }

    #[test]
    fn the_grabber_rejects_empty_and_oversized_regions() {
        assert!(BackdropGrabber::new(0, 10).is_none());
        assert!(BackdropGrabber::new(10, 0).is_none());
        assert!(BackdropGrabber::new(9_000, 10).is_none());
    }

    #[test]
    fn capture_backdrop_now_rejects_empty_and_oversized_regions() {
        assert!(capture_backdrop_now((0, 0), 0, 10).is_none());
        assert!(capture_backdrop_now((0, 0), 10, 0).is_none());
        assert!(capture_backdrop_now((0, 0), 9_000, 10).is_none());
    }

    #[test]
    fn is_surface_visible_returns_false_for_null_handle() {
        assert!(!is_surface_visible(0));
    }

    #[test]
    fn capture_wallpaper_now_rejects_empty_and_oversized_regions() {
        assert!(capture_wallpaper_now((0, 0), 0, 10).is_none());
        assert!(capture_wallpaper_now((0, 0), 10, 0).is_none());
        assert!(capture_wallpaper_now((0, 0), 9_000, 10).is_none());
    }

    #[test]
    fn crop_bgra_region_extracts_interior_and_clamps_out_of_bounds() {
        let source = vec![
            0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255,
        ];
        let interior = crop_bgra_region(&source, 2, 2, (1, 0), 1, 1).expect("interior crop");
        let p = interior.pixel(0, 0);
        assert!(p[0] < 0.01 && p[1] > 0.99 && p[2] < 0.01);
        let clamped = crop_bgra_region(&source, 2, 2, (-3, 5), 2, 1).expect("clamped crop");
        for x in 0..2 {
            let c = clamped.pixel(x, 0);
            assert!(c[0] < 0.01 && c[1] < 0.01 && c[2] > 0.99);
        }
    }

    mod native {
        use std::mem::size_of;
        use std::time::Duration;

        use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
        use windows::Win32::Graphics::Gdi::CreateSolidBrush;
        use windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE,
            PeekMessageW, RegisterClassExW, SW_SHOWNOACTIVATE, ShowWindow, TranslateMessage,
            WNDCLASSEXW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        };
        use windows::core::{PCWSTR, w};

        use super::super::{
            BackdropGrabber, BackdropRequest, exclude_from_capture, include_in_capture,
        };

        const MAGENTA: COLORREF = COLORREF(0x00FF_00FF);

        fn pump(milliseconds: u64) {
            let end = std::time::Instant::now() + Duration::from_millis(milliseconds);
            let mut message = MSG::default();
            while std::time::Instant::now() < end {
                unsafe {
                    while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }

        unsafe extern "system" fn fixture_proc(
            hwnd: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }

        fn open_magenta_window(x: i32, y: i32) -> HWND {
            unsafe {
                let instance = GetModuleHandleW(None).unwrap().into();
                let class = w!("OttoBackdropFixture");
                let class_info = WNDCLASSEXW {
                    cbSize: size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(fixture_proc),
                    hInstance: instance,
                    lpszClassName: class,
                    hbrBackground: CreateSolidBrush(MAGENTA),
                    ..WNDCLASSEXW::default()
                };
                RegisterClassExW(&class_info);
                let hwnd = CreateWindowExW(
                    WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    class,
                    PCWSTR::null(),
                    WS_POPUP,
                    x,
                    y,
                    200,
                    120,
                    None,
                    None,
                    Some(instance),
                    None,
                )
                .unwrap();
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                hwnd
            }
        }

        fn centre_is_magenta(grabber: &BackdropGrabber, request: &BackdropRequest) -> bool {
            let frame = grabber
                .grab(request)
                .expect("the screen should be readable");
            let pixel = frame.pixels.pixel(50, 30);
            pixel[0] > 0.95 && pixel[1] < 0.05 && pixel[2] > 0.95
        }

        #[test]
        #[ignore = "native fixture: opens a real window on the desktop"]
        fn an_excluded_window_disappears_from_the_capture_and_returns_when_included() {
            crate::screen::enable_per_monitor_dpi();
            let (x, y) = (300, 300);
            let hwnd = open_magenta_window(x, y);
            let request = BackdropRequest {
                id: 1,
                origin: (x, y),
                width: 100,
                height: 60,
            };
            let grabber = BackdropGrabber::new(request.width, request.height).unwrap();
            pump(300);
            let visible_before = centre_is_magenta(&grabber, &request);
            let excluded = exclude_from_capture(hwnd.0 as isize);
            pump(300);
            let visible_while_excluded = centre_is_magenta(&grabber, &request);
            let included = include_in_capture(hwnd.0 as isize);
            pump(300);
            let visible_after = centre_is_magenta(&grabber, &request);
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            assert!(excluded && included);
            assert!(
                visible_before,
                "the window should be captured before exclusion"
            );
            assert!(
                !visible_while_excluded,
                "the window should be absent while excluded"
            );
            assert!(
                visible_after,
                "the window should be captured again after inclusion"
            );
        }
    }
}
