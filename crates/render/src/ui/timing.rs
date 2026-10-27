// SPDX-License-Identifier: GPL-3.0-or-later
//! GPU time of the UI passes from timestamp queries ([`super::UiStats::gpu_ns`],
//! `render.md` §6.16 U2).
//!
//! Only where the device was created with [`wgpu::Features::TIMESTAMP_QUERY`]
//! (`context.rs` requests it when the adapter offers it). Every UI render pass of a frame
//! (the passes between ordinary passes and the software cursor) writes a timestamp at its
//! beginning and its end; after the last UI draw of the frame the pairs are resolved and
//! copied into one of a few read-back buffers. The buffer is mapped at the start of a later
//! frame's preparation, once the frame that filled it has been submitted, and read without
//! blocking once its mapping completes. A frame's figure is the sum over its UI passes of
//! end − beginning, converted with the queue's timestamp period, so the value reported is
//! the one of the most recent frame whose queries have resolved: it lags by a few frames,
//! as §6.16 allows. Frames with more UI passes than [`MAX_TIMED_PASSES`], or that find no
//! free read-back buffer, are not timed (the previous figure stays).
//!
//! The resolve is encoded before the frame's commands are submitted, and the renderer can
//! still give the frame up after it (a failed UI texture flush drops the encoder). A buffer
//! whose frame may not have been submitted is therefore never read: the frame records the
//! number of failed UI flushes when it resolves ([`UiGpuTimer::finish_frame`]), and the next
//! frame frees its buffer unread when that number has changed since
//! ([`UiGpuTimer::begin_frame`]). A failed flush between that frame's resolve and its
//! preparation can cost one figure (conservative); a stale or zero figure is never reported.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// Most UI passes (cursor included) one frame times; a frame with more is not timed.
pub(crate) const MAX_TIMED_PASSES: u32 = 64;

/// Read-back buffers in flight (a frame that finds none free is not timed).
const READBACK_BUFFERS: usize = 4;

/// Mapping states of a read-back buffer's `map_async` callback.
const MAP_WAITING: u8 = 0;
const MAP_DONE: u8 = 1;
const MAP_FAILED: u8 = 2;

/// Where a read-back buffer is in its cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    /// Unmapped and unused: free for the next frame.
    Idle,
    /// Filled by the commands of frame number `seq` with `pairs` timestamp pairs; not
    /// mapped yet. `failures` is the number of failed UI flushes when it was encoded.
    Recorded { pairs: u32, seq: u64, failures: u64 },
    /// `map_async` called; waiting for its callback.
    Mapping { pairs: u32, seq: u64 },
}

#[derive(Debug)]
struct Slot {
    buffer: wgpu::Buffer,
    state: SlotState,
    map: Arc<AtomicU8>,
}

#[derive(Debug)]
struct Inner {
    slots: Vec<Slot>,
    /// The slot this frame resolves into, chosen in [`UiGpuTimer::begin_frame`].
    frame_slot: Option<usize>,
    /// Timestamp pairs written by this frame so far.
    passes: u32,
    /// The frame had more UI passes than [`MAX_TIMED_PASSES`].
    overflow: bool,
    /// Number of the next frame that resolves its queries.
    next_seq: u64,
    /// The most recent figure and the number of the frame it belongs to.
    latest: Option<(u64, u64)>,
}

/// The UI pass timer of a device with timestamp queries.
#[derive(Debug)]
pub(crate) struct UiGpuTimer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    /// Nanoseconds per timestamp tick.
    period_ns: f64,
    inner: Mutex<Inner>,
}

impl UiGpuTimer {
    /// A timer for `device`, or `None` when the device lacks `TIMESTAMP_QUERY`.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let count = 2 * MAX_TIMED_PASSES;
        let bytes = u64::from(count) * wgpu::QUERY_SIZE as u64;
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("tessera-ui-timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count,
        });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tessera-ui-timestamp-resolve"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let slots = (0..READBACK_BUFFERS)
            .map(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("tessera-ui-timestamp-readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                state: SlotState::Idle,
                map: Arc::new(AtomicU8::new(MAP_WAITING)),
            })
            .collect();
        let period_ns = f64::from(queue.get_timestamp_period());
        tracing::debug!(period_ns, "UI pass GPU timing on (timestamp queries)");
        Some(Self {
            set,
            resolve,
            period_ns: if period_ns.is_finite() && period_ns > 0.0 {
                period_ns
            } else {
                1.0
            },
            inner: Mutex::new(Inner {
                slots,
                frame_slot: None,
                passes: 0,
                overflow: false,
                next_seq: 0,
                latest: None,
            }),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Starts a frame: maps the buffers earlier frames filled, reads those whose mapping has
    /// completed (without blocking), chooses this frame's buffer, and returns the most recent
    /// figure. `failed_flushes` is the renderer's count of failed UI flushes so far
    /// (`PendingOps::failed_flushes`): a buffer filled while the count was different belongs
    /// to a frame that may never have been submitted, and is freed unread.
    pub(crate) fn begin_frame(&self, device: &wgpu::Device, failed_flushes: u64) -> Option<u64> {
        let mut inner = self.lock();
        let mut mapping = false;
        for slot in &mut inner.slots {
            if let SlotState::Recorded {
                pairs,
                seq,
                failures,
            } = slot.state
            {
                if failures != failed_flushes {
                    tracing::debug!(
                        seq,
                        "UI pass GPU timing: frame possibly not submitted, figure dropped"
                    );
                    slot.state = SlotState::Idle;
                    continue;
                }
                slot.map.store(MAP_WAITING, Ordering::Release);
                let flag = Arc::clone(&slot.map);
                slot.buffer.map_async(wgpu::MapMode::Read, .., move |r| {
                    flag.store(
                        if r.is_ok() { MAP_DONE } else { MAP_FAILED },
                        Ordering::Release,
                    );
                });
                slot.state = SlotState::Mapping { pairs, seq };
            }
            mapping |= matches!(slot.state, SlotState::Mapping { .. });
        }
        if mapping && let Err(e) = device.poll(wgpu::PollType::Poll) {
            tracing::debug!(error = %e, "UI pass GPU timing: device poll failed");
        }
        // Read the buffers whose mapping has completed; the figure of the most recent frame
        // wins, whatever order the buffers finish in.
        let period = self.period_ns;
        let mut latest = inner.latest;
        for slot in &mut inner.slots {
            let SlotState::Mapping { pairs, seq } = slot.state else {
                continue;
            };
            match slot.map.load(Ordering::Acquire) {
                MAP_DONE => {
                    let figure = read_pairs(&slot.buffer, pairs, period);
                    slot.buffer.unmap();
                    slot.state = SlotState::Idle;
                    if let Some(ns) = figure
                        && latest.is_none_or(|(_, s)| seq >= s)
                    {
                        latest = Some((ns, seq));
                    }
                }
                MAP_FAILED => slot.state = SlotState::Idle,
                _ => {}
            }
        }
        inner.latest = latest;
        inner.frame_slot = inner.slots.iter().position(|s| s.state == SlotState::Idle);
        inner.passes = 0;
        inner.overflow = false;
        inner.latest.map(|(ns, _)| ns)
    }

    /// The timestamp writes of the next UI render pass of this frame, or `None` when the
    /// frame is not timed.
    pub(crate) fn pass_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let mut inner = self.lock();
        inner.frame_slot?;
        if inner.passes >= MAX_TIMED_PASSES {
            inner.overflow = true;
            return None;
        }
        let k = inner.passes;
        inner.passes += 1;
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(2 * k),
            end_of_pass_write_index: Some(2 * k + 1),
        })
    }

    /// Ends the frame's UI work: resolves its timestamp pairs into the frame's read-back
    /// buffer. A frame without UI passes has the figure 0 at once. `failed_flushes` is the
    /// renderer's count of failed UI flushes so far ([`UiGpuTimer::begin_frame`]).
    pub(crate) fn finish_frame(&self, encoder: &mut wgpu::CommandEncoder, failed_flushes: u64) {
        let mut inner = self.lock();
        let Some(k) = inner.frame_slot.take() else {
            return;
        };
        let pairs = inner.passes;
        if inner.overflow {
            return;
        }
        let seq = inner.next_seq;
        inner.next_seq = seq.wrapping_add(1);
        if pairs == 0 {
            inner.latest = Some((0, seq));
            return;
        }
        let Some(slot) = inner.slots.get_mut(k) else {
            return;
        };
        let bytes = u64::from(2 * pairs) * wgpu::QUERY_SIZE as u64;
        encoder.resolve_query_set(&self.set, 0..2 * pairs, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &slot.buffer, 0, bytes);
        slot.state = SlotState::Recorded {
            pairs,
            seq,
            failures: failed_flushes,
        };
    }
}

/// The summed GPU time of `pairs` (beginning, end) timestamp pairs in a mapped buffer, in
/// nanoseconds; `None` when the buffer cannot be read. A pair whose end lies before its
/// beginning (a counter reset) adds nothing.
fn read_pairs(buffer: &wgpu::Buffer, pairs: u32, period_ns: f64) -> Option<u64> {
    let bytes = u64::from(2 * pairs) * wgpu::QUERY_SIZE as u64;
    let view = buffer.get_mapped_range(..bytes).ok()?;
    let mut ticks: u64 = 0;
    for pair in view.as_chunks::<16>().0 {
        let (b, e) = pair.split_at(8);
        let begin = u64::from_le_bytes(b.try_into().ok()?);
        let end = u64::from_le_bytes(e.try_into().ok()?);
        ticks = ticks.saturating_add(end.saturating_sub(begin));
    }
    drop(view);
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let ns = (ticks as f64 * period_ns).round() as u64;
    Some(ns)
}

#[cfg(test)]
impl UiGpuTimer {
    /// The frame number of the most recent figure (tests).
    fn latest_seq(&self) -> Option<u64> {
        self.lock().latest.map(|(_, seq)| seq)
    }

    /// How many read-back buffers wait for (or are in) a mapping (tests).
    fn pending_slots(&self) -> usize {
        self.lock()
            .slots
            .iter()
            .filter(|s| s.state != SlotState::Idle)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{ContextOptions, GpuContext};

    /// A device with timestamp queries, or `None` (the test is skipped).
    fn timed_context() -> Option<GpuContext> {
        let options = ContextOptions::default();
        let instance = GpuContext::create_instance(&options, None);
        let ctx = GpuContext::new_with_instance(instance, &options, None).ok()?;
        ctx.device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then_some(ctx)
    }

    /// One frame with one timed (empty) UI pass, resolved into `encoder`.
    fn timed_frame(
        ctx: &GpuContext,
        timer: &UiGpuTimer,
        target: &wgpu::TextureView,
        failed_flushes: u64,
    ) -> (Option<u64>, wgpu::CommandEncoder) {
        let figure = timer.begin_frame(&ctx.device, failed_flushes);
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("timer-test"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: timer.pass_writes(),
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        timer.finish_frame(&mut encoder, failed_flushes);
        (figure, encoder)
    }

    /// A frame whose encoder is dropped after its resolve (a failed UI flush before the
    /// submit) never has its read-back buffer read: no stale or zero figure becomes the
    /// newest one.
    #[test]
    fn a_frame_given_up_after_its_resolve_is_not_read() {
        // First, so that every GPU object below is dropped before the lock.
        let _gpu = crate::context::gpu_test_lock();
        let Some(ctx) = timed_context() else {
            eprintln!("SKIPPED: no adapter with TIMESTAMP_QUERY");
            return;
        };
        let Some(timer) = UiGpuTimer::new(&ctx.device, &ctx.queue) else {
            return;
        };
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("timer-test"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Reads every mapped buffer (starting extra frames, which are not timed).
        let settle = |failed_flushes: u64| {
            for _ in 0..16 {
                let _ = timer.begin_frame(&ctx.device, failed_flushes);
                if timer.pending_slots() == 0 {
                    break;
                }
                let _ = ctx.device.poll(crate::context::bounded_wait());
            }
        };
        // Frame 0 is submitted and read.
        let (_, enc) = timed_frame(&ctx, &timer, &view, 0);
        ctx.queue.submit(Some(enc.finish()));
        settle(0);
        assert_eq!(timer.pending_slots(), 0);
        let first = timer.latest_seq();
        assert!(first.is_some(), "frame 0 has a figure");
        // The next frame is given up after its resolve (the flush before its submit failed).
        let (_, dropped) = timed_frame(&ctx, &timer, &view, 0);
        drop(dropped);
        assert_eq!(
            timer.pending_slots(),
            1,
            "the given-up frame's buffer is recorded"
        );
        // The frame after it sees the failed flush: that buffer is freed unread.
        let (_, enc) = timed_frame(&ctx, &timer, &view, 1);
        assert_eq!(
            timer.pending_slots(),
            1,
            "only this frame's own buffer is pending"
        );
        assert_eq!(
            timer.latest_seq(),
            first,
            "no figure from the given-up frame"
        );
        ctx.queue.submit(Some(enc.finish()));
        settle(1);
        let last = timer.latest_seq();
        assert!(
            last.zip(first).is_some_and(|(l, f)| l >= f + 2),
            "the submitted frame's figure arrives: {first:?} then {last:?}"
        );
    }
}
