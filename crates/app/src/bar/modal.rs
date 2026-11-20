use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::host::RuntimeContext;
use otto_core::{NotchPort, SHELF_THUMBNAIL_MAX_PX, ShelfPayload, ShelfThumbnail};
use otto_platform::release_capture_if_owned;
use otto_platform::{ShelfDropEvent, WindowsNotchPort};

use super::with_bar;
use super::with_bar_result;

pub(crate) enum DeferredModalAction {
    PickShelfFiles,
    StartShelfDrag {
        capture_hwnd: isize,
        payload: ShelfPayload,
    },
}

pub(crate) enum PreparedModalAction {
    PickShelfFiles {
        port: WindowsNotchPort,
    },
    StartShelfDrag {
        capture_hwnd: isize,
        payload: ShelfPayload,
        local_drag: Arc<AtomicBool>,
        port: WindowsNotchPort,
    },
}

pub(crate) enum CompletedModalAction {
    PickedShelfFiles(Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>),
    FinishedShelfDrag,
}

pub(super) struct LocalDragGuard(Arc<AtomicBool>);

impl Drop for LocalDragGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub(super) enum ProcessedShelfEvent {
    DragEnter,
    DragLeave,
    Drop(Vec<(ShelfPayload, bool, Option<ShelfThumbnail>)>),
}

pub(super) fn handle_shelf_drop_events(runtime: &RuntimeContext) {
    let Some((shelf_enabled, queue, port)) = with_bar_result(runtime, |bar| {
        (
            bar.notch_enabled && bar.notch.shelf_enabled(),
            bar.io.shelf_queue.clone(),
            bar.notch_port,
        )
    }) else {
        return;
    };
    let events = queue.drain();
    if events.is_empty() || !shelf_enabled {
        return;
    }
    let processed: Vec<ProcessedShelfEvent> = events
        .into_iter()
        .map(|ev| match ev {
            ShelfDropEvent::DragEnter => ProcessedShelfEvent::DragEnter,
            ShelfDropEvent::DragLeave => ProcessedShelfEvent::DragLeave,
            ShelfDropEvent::Drop(payloads) => {
                let enriched = payloads
                    .into_iter()
                    .map(|payload| match &payload {
                        ShelfPayload::File(path) => {
                            let (is_dir, thumb) =
                                port.inspect_shelf_file(path, SHELF_THUMBNAIL_MAX_PX);
                            (payload, is_dir, thumb)
                        }
                        ShelfPayload::Text(_) | ShelfPayload::Url(_) => (payload, false, None),
                    })
                    .collect();
                ProcessedShelfEvent::Drop(enriched)
            }
        })
        .collect();
    with_bar(runtime, |bar| {
        bar.ensure_anim_timer();
        let now = Instant::now();
        let mut changed = false;
        for ev in processed {
            let update = match ev {
                ProcessedShelfEvent::DragEnter => bar.notch.handle_shelf_drag_enter(),
                ProcessedShelfEvent::DragLeave => bar.notch.handle_shelf_drag_leave(now),
                ProcessedShelfEvent::Drop(items) => bar.notch.handle_shelf_drop(items),
            };
            changed |= update.changed;
        }
        if changed {
            bar.recompute_layout();
            bar.sync_panel_visibility();
            bar.present(runtime);
        }
        bar.schedule_deadline();
    });
}

pub(super) fn queue_modal_action(runtime: &RuntimeContext, action: DeferredModalAction) {
    if let Ok(mut pending) = runtime.pending_modal_action.try_borrow_mut()
        && pending.is_none()
    {
        *pending = Some(action);
    }
}

pub(crate) fn prepare_modal_action(
    runtime: &RuntimeContext,
    action: DeferredModalAction,
) -> Option<PreparedModalAction> {
    match action {
        DeferredModalAction::PickShelfFiles => {
            let port = with_bar_result(runtime, |bar| {
                if !bar.notch.begin_shelf_modal_picker() {
                    return None;
                }
                bar.recompute_layout();
                bar.sync_panel_visibility();
                bar.present(runtime);
                Some(bar.notch_port)
            })
            .flatten()?;
            Some(PreparedModalAction::PickShelfFiles { port })
        }
        DeferredModalAction::StartShelfDrag {
            capture_hwnd,
            payload,
        } => {
            let (local_drag, port) = with_bar_result(runtime, |bar| {
                let _ = bar.notch.set_shelf_drag_out(true);
                (Arc::clone(&bar.io.shelf_local_drag), bar.notch_port)
            })?;
            Some(PreparedModalAction::StartShelfDrag {
                capture_hwnd,
                payload,
                local_drag,
                port,
            })
        }
    }
}

pub(crate) fn run_prepared_modal_action(action: PreparedModalAction) -> CompletedModalAction {
    match action {
        PreparedModalAction::PickShelfFiles { port } => {
            let picked = port.pick_shelf_files();
            let enriched = picked
                .into_iter()
                .map(|path| {
                    let (is_dir, thumb) = port.inspect_shelf_file(&path, SHELF_THUMBNAIL_MAX_PX);
                    (ShelfPayload::File(path), is_dir, thumb)
                })
                .collect();
            CompletedModalAction::PickedShelfFiles(enriched)
        }
        PreparedModalAction::StartShelfDrag {
            capture_hwnd,
            payload,
            local_drag,
            port,
        } => {
            let _ = release_capture_if_owned(capture_hwnd);
            local_drag.store(true, Ordering::Release);
            let guard = LocalDragGuard(local_drag);
            let _ = port.start_shelf_drag(capture_hwnd, &payload);
            drop(guard);
            CompletedModalAction::FinishedShelfDrag
        }
    }
}

pub(crate) fn finish_modal_action(runtime: &RuntimeContext, completion: CompletedModalAction) {
    with_bar(runtime, |bar| {
        bar.ensure_anim_timer();
        match completion {
            CompletedModalAction::PickedShelfFiles(enriched) => {
                let update = bar.notch.finish_shelf_modal_picker(enriched);
                if update.changed {
                    bar.recompute_layout();
                    bar.sync_panel_visibility();
                    bar.present(runtime);
                }
            }
            CompletedModalAction::FinishedShelfDrag => {
                let _ = bar.notch.set_shelf_drag_out(false);
                bar.recompute_layout();
                bar.sync_panel_visibility();
                bar.present(runtime);
            }
        }
        bar.schedule_deadline();
    });
}
