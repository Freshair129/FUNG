//! Private SPSC ring for routed playback. This module grants no media access.
//!
//! Exactly one worker owns `Writer`, and exactly one audio callback owns
//! `Reader`. `Control` may be cloned for control threads. Neither ring endpoint
//! allocates, blocks, or locks in `push`/`poll`. Old epochs may remain in the
//! queue until the callback discards them, including while playback is paused.
//!
//! A returned frame is NOT permission to emit: the caller must hold the existing
//! playback lease's output fence, recheck quarantine/epoch while fenced, and
//! complete the callback before acknowledging seek/pause/revocation. The ring
//! cannot recall samples submitted to the device. The worker must hold the
//! separate producer fence across publication, and discard a decoded batch if
//! a seek invalidates it. These rules are deliberately enforced by the player,
//! not by exposing or duplicating the custody lease here.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

/// Reserve 64 KiB of the 1 MiB queue budget for worker staging and 4 KiB for
/// queue/control objects. This is a retained logical allocation budget, not a
/// guarantee about allocator bookkeeping or the unrelated SDK memory peak.
pub(crate) const MAX_STAGING_BYTES: usize = 64 * 1024;
pub(crate) const CONTROL_HEADROOM_BYTES: usize = 4 * 1024;
pub(crate) const MAX_RING_BYTES: usize = 1024 * 1024 - MAX_STAGING_BYTES - CONTROL_HEADROOM_BYTES;
pub(crate) const MAX_FRAMES: usize = MAX_RING_BYTES / std::mem::size_of::<TaggedFrame>();
/// Avoid unbounded real-time work after a seek with a queue full of stale data.
pub(crate) const MAX_DISCARD_PER_POLL: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TaggedFrame {
    pub(crate) epoch: u64,
    pub(crate) samples: [i16; 2],
}

#[derive(Debug)]
struct Shared {
    epoch: AtomicU64,
    revoked: AtomicBool,
}

#[derive(Clone, Debug)]
pub(crate) struct Control(Arc<Shared>);

pub(crate) struct Writer {
    inner: rtrb::Producer<TaggedFrame>,
    control: Control,
}

pub(crate) struct Reader {
    inner: rtrb::Consumer<TaggedFrame>,
    control: Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Push {
    Published,
    Full,
    Stale,
    Invalidated,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Poll {
    Frame(TaggedFrame),
    Empty,
    Paused,
    DrainPending,
    Revoked,
}

pub(crate) fn channel(capacity: usize) -> Result<(Control, Writer, Reader), &'static str> {
    if capacity == 0 || capacity > MAX_FRAMES {
        return Err("invalid routed queue capacity");
    }
    let control = Control(Arc::new(Shared {
        epoch: AtomicU64::new(1),
        revoked: AtomicBool::new(false),
    }));
    let (writer, reader) = rtrb::RingBuffer::new(capacity);
    Ok((
        control.clone(),
        Writer {
            inner: writer,
            control: control.clone(),
        },
        Reader {
            inner: reader,
            control,
        },
    ))
}

impl Control {
    pub(crate) fn current_epoch(&self) -> u64 {
        self.0.epoch.load(Ordering::Acquire)
    }
    pub(crate) fn is_revoked(&self) -> bool {
        self.0.revoked.load(Ordering::Acquire)
    }
    pub(crate) fn revoke(&self) {
        self.0.revoked.store(true, Ordering::Release);
    }
    pub(crate) fn advance_epoch(&self) -> Result<u64, &'static str> {
        loop {
            if self.is_revoked() {
                return Err("source revoked");
            }
            let old = self.current_epoch();
            let Some(next) = old.checked_add(1) else {
                self.revoke();
                return Err("epoch exhausted");
            };
            if self
                .0
                .epoch
                .compare_exchange(old, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return if self.is_revoked() {
                    Err("source revoked")
                } else {
                    Ok(next)
                };
            }
        }
    }
}

impl Writer {
    /// A lower-bound snapshot of free frames. The sole consumer can only free
    /// more slots, but callers must recheck epoch/revocation around publication.
    pub(crate) fn free_slots(&self) -> usize {
        if self.control.is_revoked() {
            0
        } else {
            self.inner.slots()
        }
    }

    pub(crate) fn push(&mut self, epoch: u64, samples: [i16; 2]) -> Push {
        self.push_impl(epoch, samples, || {})
    }

    #[cfg(test)]
    fn push_with_hook(&mut self, epoch: u64, samples: [i16; 2], hook: impl FnOnce()) -> Push {
        self.push_impl(epoch, samples, hook)
    }

    fn push_impl(&mut self, epoch: u64, samples: [i16; 2], hook: impl FnOnce()) -> Push {
        if self.control.is_revoked() {
            return Push::Revoked;
        }
        if self.control.current_epoch() != epoch {
            return Push::Stale;
        }
        hook();
        if self.inner.push(TaggedFrame { epoch, samples }).is_err() {
            return if self.control.is_revoked() {
                Push::Revoked
            } else if self.control.current_epoch() != epoch {
                Push::Stale
            } else {
                Push::Full
            };
        }
        if self.control.is_revoked() || self.control.current_epoch() != epoch {
            // The item can be in the ring; Reader will never return it as current.
            Push::Invalidated
        } else {
            Push::Published
        }
    }
}

impl Reader {
    /// Discards up to `min(discard_budget, MAX_DISCARD_PER_POLL)` frames known
    /// stale at peek time. It may additionally pop one current candidate that a
    /// concurrent epoch change makes stale. Thus each call performs at most the
    /// capped budget plus one pops, including a successfully returned frame;
    /// budget zero can still pop exactly one raced candidate. On paused playback
    /// current-epoch frames stay queued; stale frames drain.
    pub(crate) fn poll(&mut self, paused: bool, discard_budget: usize) -> Poll {
        self.poll_impl(paused, discard_budget, || {})
    }

    #[cfg(test)]
    fn poll_with_hook(&mut self, paused: bool, discard_budget: usize, hook: impl FnOnce()) -> Poll {
        self.poll_impl(paused, discard_budget, hook)
    }

    fn poll_impl(&mut self, paused: bool, discard_budget: usize, hook: impl FnOnce()) -> Poll {
        let mut remaining = discard_budget.min(MAX_DISCARD_PER_POLL);
        let mut hook = Some(hook);
        loop {
            if self.control.is_revoked() {
                return Poll::Revoked;
            }
            let frame = match self.inner.peek() {
                Ok(frame) => *frame,
                Err(_) => return if paused { Poll::Paused } else { Poll::Empty },
            };
            let epoch = self.control.current_epoch();
            if frame.epoch != epoch {
                if remaining == 0 {
                    return Poll::DrainPending;
                }
                let _ = self.inner.pop();
                remaining -= 1;
                continue;
            }
            if paused {
                return Poll::Paused;
            }
            // After pop, recheck epoch: a raced seek makes the frame stale. The
            // player's output fence is still required for races after return.
            let frame = match self.inner.pop() {
                Ok(frame) => frame,
                Err(_) => return Poll::Empty,
            };
            if let Some(hook) = hook.take() {
                hook();
            }
            if self.control.is_revoked() {
                return Poll::Revoked;
            }
            if frame.epoch != self.control.current_epoch() {
                if remaining == 0 {
                    return Poll::DrainPending;
                }
                remaining -= 1;
                continue;
            }
            return Poll::Frame(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_is_bounded_and_full_does_not_overwrite() {
        let (control, mut producer, mut consumer) = channel(2).unwrap();
        assert!(channel(MAX_FRAMES + 1).is_err());
        assert!(MAX_FRAMES * std::mem::size_of::<TaggedFrame>() <= MAX_RING_BYTES);
        let static_control_bytes = std::mem::size_of::<rtrb::RingBuffer<TaggedFrame>>()
            + std::mem::size_of::<Writer>()
            + std::mem::size_of::<Reader>()
            + std::mem::size_of::<Shared>()
            + 4 * std::mem::size_of::<usize>();
        assert!(static_control_bytes <= CONTROL_HEADROOM_BYTES);
        assert!(
            MAX_FRAMES * std::mem::size_of::<TaggedFrame>()
                + MAX_STAGING_BYTES
                + CONTROL_HEADROOM_BYTES
                <= 1024 * 1024
        );
        let epoch = control.current_epoch();
        assert_eq!(producer.free_slots(), 2);
        assert_eq!(producer.push(epoch, [1, 2]), Push::Published);
        assert_eq!(producer.free_slots(), 1);
        assert_eq!(producer.push(epoch, [3, 4]), Push::Published);
        assert_eq!(producer.push(epoch, [5, 6]), Push::Full);
        assert_eq!(producer.free_slots(), 0);
        assert_eq!(
            consumer.poll(false, 4),
            Poll::Frame(TaggedFrame {
                epoch,
                samples: [1, 2]
            })
        );
        assert_eq!(
            consumer.poll(false, 4),
            Poll::Frame(TaggedFrame {
                epoch,
                samples: [3, 4]
            })
        );
        assert_eq!(consumer.poll(false, 4), Poll::Empty);
    }

    #[test]
    fn paused_consumer_discards_stale_in_bounded_steps_without_eating_new_epoch() {
        let (control, mut producer, mut consumer) = channel(4).unwrap();
        let old = control.current_epoch();
        for _ in 0..3 {
            assert_eq!(producer.push(old, [1, 1]), Push::Published);
        }
        let new = control.advance_epoch().unwrap();
        assert_eq!(producer.push(new, [2, 2]), Push::Published);
        assert_eq!(consumer.poll(true, 2), Poll::DrainPending);
        assert_eq!(consumer.poll(true, 2), Poll::Paused);
        assert_eq!(
            consumer.poll(false, 2),
            Poll::Frame(TaggedFrame {
                epoch: new,
                samples: [2, 2]
            })
        );
    }

    #[test]
    fn seek_between_validation_and_publication_cannot_emit_old_epoch() {
        let (control, mut producer, mut consumer) = channel(2).unwrap();
        let old = control.current_epoch();
        let advance = control.clone();
        assert_eq!(
            producer.push_with_hook(old, [7, 7], || {
                advance.advance_epoch().unwrap();
            }),
            Push::Invalidated
        );
        assert_eq!(consumer.poll(false, 2), Poll::Empty);
        let new = control.current_epoch();
        assert_eq!(producer.push(new, [8, 8]), Push::Published);
        assert_eq!(
            consumer.poll(false, 2),
            Poll::Frame(TaggedFrame {
                epoch: new,
                samples: [8, 8]
            })
        );
    }

    #[test]
    fn seek_after_pop_with_zero_discard_budget_drops_one_raced_frame() {
        let (control, mut producer, mut consumer) = channel(2).unwrap();
        let old = control.current_epoch();
        assert_eq!(producer.push(old, [7, 7]), Push::Published);
        let advance = control.clone();
        assert_eq!(
            consumer.poll_with_hook(false, 0, || {
                advance.advance_epoch().unwrap();
            }),
            Poll::DrainPending
        );
        let new = control.current_epoch();
        assert_eq!(producer.push(new, [8, 8]), Push::Published);
        assert_eq!(
            consumer.poll(false, 0),
            Poll::Frame(TaggedFrame {
                epoch: new,
                samples: [8, 8]
            })
        );
    }

    #[test]
    fn revoke_denies_pending_and_future_frames() {
        let (control, mut producer, mut consumer) = channel(2).unwrap();
        let epoch = control.current_epoch();
        assert_eq!(producer.push(epoch, [9, 9]), Push::Published);
        control.revoke();
        assert_eq!(consumer.poll(false, 2), Poll::Revoked);
        assert_eq!(producer.push(epoch, [3, 3]), Push::Revoked);
        assert!(control.advance_epoch().is_err());
    }

    #[test]
    fn revoke_between_validation_and_publication_is_not_emitted() {
        let (control, mut producer, mut consumer) = channel(2).unwrap();
        let epoch = control.current_epoch();
        let revoke = control.clone();
        assert_eq!(
            producer.push_with_hook(epoch, [4, 4], || revoke.revoke()),
            Push::Invalidated
        );
        assert_eq!(consumer.poll(false, 2), Poll::Revoked);
    }

    #[test]
    fn full_queue_racing_seek_reports_stale_instead_of_waiting_for_space() {
        let (control, mut producer, mut consumer) = channel(1).unwrap();
        let old = control.current_epoch();
        assert_eq!(producer.push(old, [1, 1]), Push::Published);
        let advance = control.clone();
        assert_eq!(
            producer.push_with_hook(old, [2, 2], || {
                advance.advance_epoch().unwrap();
            }),
            Push::Stale
        );
        assert_eq!(consumer.poll(false, 2), Poll::Empty);
    }

    #[test]
    fn full_old_epoch_queue_is_drained_while_paused_before_new_frames() {
        let (control, mut producer, mut consumer) = channel(4).unwrap();
        let old = control.current_epoch();
        for _ in 0..4 {
            assert_eq!(producer.push(old, [1, 1]), Push::Published);
        }
        let new = control.advance_epoch().unwrap();
        assert_eq!(producer.push(new, [2, 2]), Push::Full);
        assert_eq!(consumer.poll(true, 2), Poll::DrainPending);
        assert_eq!(consumer.poll(true, 2), Poll::Paused);
        assert_eq!(producer.push(new, [2, 2]), Push::Published);
        assert_eq!(
            consumer.poll(false, 2),
            Poll::Frame(TaggedFrame {
                epoch: new,
                samples: [2, 2]
            })
        );
    }

    #[test]
    fn epoch_wrap_fails_closed() {
        let (control, mut producer, mut consumer) = channel(1).unwrap();
        control.0.epoch.store(u64::MAX, Ordering::Release);
        assert!(control.advance_epoch().is_err());
        assert!(control.is_revoked());
        assert_eq!(producer.push(u64::MAX, [1, 1]), Push::Revoked);
        assert_eq!(consumer.poll(false, 1), Poll::Revoked);
    }

    #[test]
    fn spsc_threads_keep_every_current_epoch_frame_in_order() {
        use std::time::{Duration, Instant};
        let (control, mut producer, mut consumer) = channel(32).unwrap();
        let epoch = control.current_epoch();
        let worker = std::thread::spawn(move || {
            for value in 0..5_000_i16 {
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    match producer.push(epoch, [value, value]) {
                        Push::Published => break,
                        Push::Full if Instant::now() < deadline => std::thread::yield_now(),
                        other => panic!("unexpected producer result {other:?}"),
                    }
                }
            }
        });
        for value in 0..5_000_i16 {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match consumer.poll(false, 16) {
                    Poll::Frame(frame) => {
                        assert_eq!(frame.samples, [value, value]);
                        break;
                    }
                    Poll::Empty if Instant::now() < deadline => std::thread::yield_now(),
                    other => panic!("unexpected consumer result {other:?}"),
                }
            }
        }
        worker.join().unwrap();
    }
}
