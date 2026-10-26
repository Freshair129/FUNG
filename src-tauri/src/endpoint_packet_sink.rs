//! Explicit future-barrier private PCM seam. No hardware or custody activation.
use std::sync::{mpsc, Arc, OnceLock};
use std::time::Duration;
use uuid::Uuid;
pub const MAX_PACKET_QUEUE_CAPACITY: usize = 64;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePacketScope {
    source_id: Uuid,
    epoch: i64,
    endpoint_id: String,
    sample_rate: u32,
    common_origin_hns: u64,
}
impl SourcePacketScope {
    /// Projection of an authoritative native registration, not an authorization
    /// token. Callers must validate registration/route ownership before activation.
    pub fn new(
        source_id: Uuid,
        epoch: i64,
        endpoint_id: String,
        sample_rate: u32,
        common_origin_hns: u64,
    ) -> Result<Self, SinkFault> {
        if source_id.is_nil()
            || epoch <= 0
            || endpoint_id.is_empty()
            || endpoint_id.len() > 4096
            || endpoint_id.contains('\0')
            || !(8000..=192000).contains(&sample_rate)
            || common_origin_hns == 0
        {
            return Err(SinkFault::Protocol(
                "invalid authoritative source projection".into(),
            ));
        }
        Ok(Self {
            source_id,
            epoch,
            endpoint_id,
            sample_rate,
            common_origin_hns,
        })
    }
    pub fn source_id(&self) -> Uuid {
        self.source_id
    }
    pub fn epoch(&self) -> i64 {
        self.epoch
    }
    pub fn endpoint_id(&self) -> &str {
        &self.endpoint_id
    }
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    pub fn common_origin_hns(&self) -> u64 {
        self.common_origin_hns
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSpan {
    pub start: u64,
    pub end: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamClock {
    pub native_origin_hns: u64,
    pub common_origin_hns: u64,
    pub sample_rate: u32,
    pub cut_frame: u64,
    pub phase_numerator: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupMissing {
    pub elapsed_hns: u64,
    pub rounded_frame_span: Option<FrameSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupUnverified {
    pub reason: String,
    pub first_observed_hns: Option<u64>,
    pub common_origin_hns: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapDiagnostic {
    pub native_frames: FrameSpan,
    pub visible_frames: Option<FrameSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BarrierWitness {
    pub native_frames: FrameSpan,
    pub native_qpc_hns: u64,
    pub observed_qpc_hns: u64,
    pub native_flags: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketStamp {
    pub first_sample: u64,
    pub silence_before: Option<u64>,
    pub native_index: u64,
    pub native_qpc_hns: u64,
    pub observed_qpc_hns: u64,
    pub native_flags: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointPacket {
    pub scope: Arc<SourcePacketScope>,
    pub frames: FrameSpan,
    pub samples: Vec<i16>,
    pub original_native_frames: FrameSpan,
    pub preroll_clipped_frames: u64,
    pub missing_before: Option<FrameSpan>,
    pub known_idle_before: Option<FrameSpan>,
    pub native_stamp: PacketStamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkFault {
    Protocol(String),
    QueueFull { dropped: FrameSpan },
    ConsumerGone { dropped: FrameSpan },
    Native(String),
    AbandonedProducer,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSummary {
    pub scope: Arc<SourcePacketScope>,
    pub clock: Option<StreamClock>,
    pub ready_observed_hns: Option<u64>,
    pub prebarrier_witness: Option<BarrierWitness>,
    pub accepted_packets: u64,
    pub last_accepted_end_frame: Option<u64>,
    pub preroll_packets_dropped: u64,
    pub preroll_frames_dropped: u64,
    pub quarantined: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SinkRead {
    StartupMissing {
        scope: Arc<SourcePacketScope>,
        missing: StartupMissing,
    },
    StartupUnverified {
        scope: Arc<SourcePacketScope>,
        unverified: StartupUnverified,
    },
    NativeGap {
        scope: Arc<SourcePacketScope>,
        gap: GapDiagnostic,
    },
    Fault {
        scope: Arc<SourcePacketScope>,
        fault: SinkFault,
    },
    Packet(EndpointPacket),
    Drained(StreamSummary),
}
struct Shared {
    clock: OnceLock<StreamClock>,
    ready: OnceLock<u64>,
    witness: OnceLock<BarrierWitness>,
    startup: OnceLock<StartupMissing>,
    unverified: OnceLock<StartupUnverified>,
    gap: OnceLock<GapDiagnostic>,
    fault: OnceLock<SinkFault>,
    summary: OnceLock<StreamSummary>,
}
pub struct EndpointPacketProducer {
    scope: Arc<SourcePacketScope>,
    sender: Option<mpsc::SyncSender<EndpointPacket>>,
    shared: Arc<Shared>,
    expected_native: u64,
    accepted_packets: u64,
    last_accepted_end: Option<u64>,
    preroll_packets: u64,
    preroll_frames: u64,
    last_observed_hns: Option<u64>,
    finished: bool,
}
pub struct EndpointPacketReceiver {
    scope: Arc<SourcePacketScope>,
    receiver: mpsc::Receiver<EndpointPacket>,
    shared: Arc<Shared>,
    reported_startup: bool,
    reported_unverified: bool,
    reported_gap: bool,
    reported_fault: bool,
    reported_drained: bool,
    pending: Option<EndpointPacket>,
}

/// Pure private stream transport. At current 48kHz, <=64 two-second PCM packets
/// bound queued samples to 12,288,000 bytes, plus bounded in-flight packets.
pub fn packet_channel(
    scope: SourcePacketScope,
    capacity: usize,
) -> Result<(EndpointPacketProducer, EndpointPacketReceiver), SinkFault> {
    if capacity == 0 || capacity > MAX_PACKET_QUEUE_CAPACITY {
        return Err(SinkFault::Protocol(
            "packet queue capacity outside bounds".into(),
        ));
    }
    let scope = Arc::new(scope);
    let shared = Arc::new(Shared {
        clock: OnceLock::new(),
        ready: OnceLock::new(),
        witness: OnceLock::new(),
        startup: OnceLock::new(),
        unverified: OnceLock::new(),
        gap: OnceLock::new(),
        fault: OnceLock::new(),
        summary: OnceLock::new(),
    });
    let (sender, receiver) = mpsc::sync_channel(capacity);
    Ok((
        EndpointPacketProducer {
            scope: scope.clone(),
            sender: Some(sender),
            shared: shared.clone(),
            expected_native: 0,
            accepted_packets: 0,
            last_accepted_end: None,
            preroll_packets: 0,
            preroll_frames: 0,
            last_observed_hns: None,
            finished: false,
        },
        EndpointPacketReceiver {
            scope,
            receiver,
            shared,
            reported_startup: false,
            reported_unverified: false,
            reported_gap: false,
            reported_fault: false,
            reported_drained: false,
            pending: None,
        },
    ))
}
fn visible_part(native: FrameSpan, cut: u64) -> Option<FrameSpan> {
    let start = native.start.max(cut);
    (native.end > start).then(|| FrameSpan {
        start: start - cut,
        end: native.end - cut,
    })
}
impl EndpointPacketProducer {
    pub fn scope(&self) -> &SourcePacketScope {
        &self.scope
    }
    fn unverified(&self, reason: &str, observed: Option<u64>) {
        let _ = self.shared.unverified.set(StartupUnverified {
            reason: reason.into(),
            first_observed_hns: observed,
            common_origin_hns: self.scope.common_origin_hns,
        });
    }
    fn complete(&mut self) {
        let _ = self.shared.summary.set(StreamSummary {
            scope: self.scope.clone(),
            clock: self.shared.clock.get().cloned(),
            ready_observed_hns: self.shared.ready.get().copied(),
            prebarrier_witness: self.shared.witness.get().cloned(),
            accepted_packets: self.accepted_packets,
            last_accepted_end_frame: self.last_accepted_end,
            preroll_packets_dropped: self.preroll_packets,
            preroll_frames_dropped: self.preroll_frames,
            quarantined: self.shared.startup.get().is_some()
                || self.shared.unverified.get().is_some()
                || self.shared.gap.get().is_some()
                || self.shared.fault.get().is_some(),
        });
        self.finished = true;
    }
    fn fail(&mut self, fault: SinkFault) -> SinkFault {
        let _ = self.shared.fault.set(fault.clone());
        self.complete();
        self.sender.take();
        fault
    }
    fn protocol(&mut self, reason: &str) -> SinkFault {
        self.fail(SinkFault::Protocol(reason.into()))
    }
    pub fn bind_native_clock(&mut self, origin: u64, rate: u32) -> Result<(), SinkFault> {
        if self.finished || self.shared.clock.get().is_some() {
            return Err(self.protocol("native clock binding must occur exactly once"));
        }
        if origin == 0 || rate != self.scope.sample_rate {
            return Err(
                self.protocol("native rate/origin does not match authoritative source projection")
            );
        }
        if origin > self.scope.common_origin_hns {
            let elapsed = origin - self.scope.common_origin_hns;
            let rounded =
                ((u128::from(elapsed) * u128::from(rate) + 5_000_000) / 10_000_000) as u64;
            let _ = self.shared.startup.set(StartupMissing {
                elapsed_hns: elapsed,
                rounded_frame_span: (rounded > 0).then_some(FrameSpan {
                    start: 0,
                    end: rounded,
                }),
            });
            return Err(
                self.protocol("native stream origin is after the shared recording deadline")
            );
        }
        let numerator = u128::from(self.scope.common_origin_hns - origin) * u128::from(rate);
        let cut = numerator.div_ceil(10_000_000);
        if cut > u128::from(rate) * 86400 {
            return Err(self.protocol("preroll barrier exceeds 24h source bound"));
        }
        let phase = cut * 10_000_000 - numerator;
        self.shared
            .clock
            .set(StreamClock {
                native_origin_hns: origin,
                common_origin_hns: self.scope.common_origin_hns,
                sample_rate: rate,
                cut_frame: cut as u64,
                phase_numerator: phase as u64,
            })
            .map_err(|_| self.protocol("native clock already bound"))?;
        Ok(())
    }
    /// Must be measured after IAudioClient::Start returns, before readiness is
    /// reported or any packet is accepted. Readiness is not a sample-clock witness.
    pub fn confirm_native_ready(&mut self, observed: u64) -> Result<(), SinkFault> {
        if self.finished || self.shared.ready.get().is_some() {
            return Err(self.protocol("native ready confirmation must occur exactly once"));
        }
        let Some(clock) = self.shared.clock.get().cloned() else {
            return Err(self.protocol("native readiness before clock binding"));
        };
        if observed < clock.native_origin_hns {
            return Err(self.protocol("native readiness observation moved backwards"));
        }
        let _ = self.shared.ready.set(observed);
        if observed > clock.common_origin_hns {
            self.unverified(
                "native Start readiness was observed after recording deadline",
                Some(observed),
            );
            return Err(
                self.protocol("native Start did not confirm readiness before shared deadline")
            );
        }
        Ok(())
    }
    pub fn push_native(
        &mut self,
        stamp: PacketStamp,
        mut samples: Vec<i16>,
    ) -> Result<(), SinkFault> {
        if self.finished {
            return Err(self
                .shared
                .fault
                .get()
                .cloned()
                .unwrap_or_else(|| SinkFault::Protocol("private packet stream closed".into())));
        }
        let Some(clock) = self.shared.clock.get().cloned() else {
            return Err(self.protocol("packet before native clock binding"));
        };
        let Some(ready) = self.shared.ready.get().copied() else {
            return Err(self.protocol("packet before post-Start readiness confirmation"));
        };
        if stamp.observed_qpc_hns < ready
            || self
                .last_observed_hns
                .is_some_and(|last| stamp.observed_qpc_hns < last)
        {
            return Err(self.protocol("packet observation clock moved backwards"));
        }
        if samples.len() > self.scope.sample_rate as usize * 2
            || samples.capacity() > self.scope.sample_rate as usize * 2
        {
            return Err(
                self.protocol("native packet length or allocation exceeds two-second bound")
            );
        }
        if stamp.first_sample < self.expected_native
            || stamp
                .silence_before
                .is_some_and(|idle| idle > stamp.first_sample)
        {
            return Err(self.protocol("native overlap or invalid idle provenance"));
        }
        if samples.is_empty() {
            return Ok(());
        }
        let Some(native_end) = stamp.first_sample.checked_add(samples.len() as u64) else {
            return Err(self.protocol("native frame overflow"));
        };
        if native_end.saturating_sub(clock.cut_frame) > u64::from(self.scope.sample_rate) * 86400 {
            return Err(self.protocol("visible packet exceeds 24h source bound"));
        }
        let native_frames = FrameSpan {
            start: stamp.first_sample,
            end: native_end,
        };
        // Legacy PacketClock clamps a negative first QPC offset to frame zero.
        // Retain that PCM privately, but do not claim the common-origin transform
        // is verified when the native anchor is known to precede its origin.
        let clamped_initial_anchor =
            self.last_observed_hns.is_none() && stamp.native_qpc_hns < clock.native_origin_hns;
        if clamped_initial_anchor {
            self.unverified(
                "first native packet QPC predates native origin; legacy frame-zero anchor is clamped",
                Some(stamp.observed_qpc_hns),
            );
        }
        if !clamped_initial_anchor
            && self.shared.witness.get().is_none()
            && stamp.observed_qpc_hns < clock.common_origin_hns
            && stamp.first_sample < clock.cut_frame
        {
            let _ = self.shared.witness.set(BarrierWitness {
                native_frames,
                native_qpc_hns: stamp.native_qpc_hns,
                observed_qpc_hns: stamp.observed_qpc_hns,
                native_flags: stamp.native_flags,
            });
        }
        let witnessed = self.shared.witness.get().is_some();
        if !witnessed {
            self.unverified(
                "no native sample-clock observation before the recording barrier",
                Some(stamp.observed_qpc_hns),
            );
        }
        // A legacy initial Some(0) may describe censored startup. Without a prebarrier
        // witness it proves neither visible silence nor a specific active-media loss.
        let censored_initial_idle =
            !witnessed && self.expected_native == 0 && stamp.silence_before == Some(0);
        let idle_start = stamp
            .silence_before
            .map(|idle| idle.max(self.expected_native));
        let missing_end = idle_start.unwrap_or(stamp.first_sample);
        let native_missing = (missing_end > self.expected_native && !censored_initial_idle)
            .then_some(FrameSpan {
                start: self.expected_native,
                end: missing_end,
            });
        if let Some(native_gap) = native_missing {
            let _ = self.shared.gap.set(GapDiagnostic {
                native_frames: native_gap,
                visible_frames: visible_part(native_gap, clock.cut_frame),
            });
        }
        let missing_before = native_missing.and_then(|gap| visible_part(gap, clock.cut_frame));
        let known_idle_before = if witnessed {
            idle_start
                .filter(|start| *start < stamp.first_sample)
                .and_then(|start| {
                    visible_part(
                        FrameSpan {
                            start,
                            end: stamp.first_sample,
                        },
                        clock.cut_frame,
                    )
                })
        } else {
            None
        };
        self.last_observed_hns = Some(stamp.observed_qpc_hns);
        let clipped = clock
            .cut_frame
            .saturating_sub(stamp.first_sample)
            .min(samples.len() as u64);
        let Some(dropped_total) = self.preroll_frames.checked_add(clipped) else {
            return Err(self.protocol("preroll counter overflow"));
        };
        if native_end <= clock.cut_frame {
            let Some(count) = self.preroll_packets.checked_add(1) else {
                return Err(self.protocol("preroll packet counter overflow"));
            };
            self.preroll_packets = count;
            self.preroll_frames = dropped_total;
            self.expected_native = native_end;
            return Ok(());
        }
        if clipped > 0 {
            samples.drain(..clipped as usize);
        }
        self.preroll_frames = dropped_total;
        let frames = FrameSpan {
            start: stamp.first_sample.max(clock.cut_frame) - clock.cut_frame,
            end: native_end - clock.cut_frame,
        };
        let Some(next_count) = self.accepted_packets.checked_add(1) else {
            return Err(self.protocol("visible packet counter overflow"));
        };
        let packet = EndpointPacket {
            scope: self.scope.clone(),
            frames,
            samples,
            original_native_frames: native_frames,
            preroll_clipped_frames: clipped,
            missing_before,
            known_idle_before,
            native_stamp: stamp,
        };
        let Some(sender) = self.sender.as_ref() else {
            return Err(self.protocol("packet queue already closed"));
        };
        match sender.try_send(packet) {
            Ok(()) => {
                self.expected_native = native_end;
                self.accepted_packets = next_count;
                self.last_accepted_end = Some(frames.end);
                self.preroll_frames = dropped_total;
                Ok(())
            }
            Err(mpsc::TrySendError::Full(_)) => {
                Err(self.fail(SinkFault::QueueFull { dropped: frames }))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                Err(self.fail(SinkFault::ConsumerGone { dropped: frames }))
            }
        }
    }
    pub fn finish(mut self, result: Result<(), String>) {
        // Preserve an actual setup/read HRESULT reason before secondary end validation.
        if let Err(reason) = result {
            let _ = self.shared.fault.set(SinkFault::Native(reason));
        }
        if self.shared.fault.get().is_none() {
            if self.shared.clock.get().is_none() {
                let _ = self
                    .shared
                    .fault
                    .set(SinkFault::Protocol("finished without native clock".into()));
            } else if self.shared.ready.get().is_none() {
                let _ = self.shared.fault.set(SinkFault::Protocol(
                    "finished without post-Start readiness confirmation".into(),
                ));
            }
            if self.shared.fault.get().is_none() {
                if self.shared.witness.get().is_none() {
                    self.unverified(
                        "source ended without a qualified prebarrier clock witness",
                        self.last_observed_hns,
                    );
                }
                if self.accepted_packets == 0 {
                    let _ = self.shared.fault.set(SinkFault::Protocol(
                        "source ended without visible PCM".into(),
                    ));
                }
            }
        }
        self.complete();
        self.sender.take();
    }
}
impl Drop for EndpointPacketProducer {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.shared.fault.set(SinkFault::AbandonedProducer);
            self.complete();
        }
    }
}
impl EndpointPacketReceiver {
    fn pending_notice(&mut self) -> Option<SinkRead> {
        if !self.reported_startup {
            if let Some(missing) = self.shared.startup.get() {
                self.reported_startup = true;
                return Some(SinkRead::StartupMissing {
                    scope: self.scope.clone(),
                    missing: missing.clone(),
                });
            }
        }
        if !self.reported_unverified {
            if let Some(unverified) = self.shared.unverified.get() {
                self.reported_unverified = true;
                return Some(SinkRead::StartupUnverified {
                    scope: self.scope.clone(),
                    unverified: unverified.clone(),
                });
            }
        }
        if !self.reported_gap {
            if let Some(gap) = self.shared.gap.get() {
                self.reported_gap = true;
                return Some(SinkRead::NativeGap {
                    scope: self.scope.clone(),
                    gap: gap.clone(),
                });
            }
        }
        if !self.reported_fault {
            if let Some(fault) = self.shared.fault.get() {
                self.reported_fault = true;
                return Some(SinkRead::Fault {
                    scope: self.scope.clone(),
                    fault: fault.clone(),
                });
            }
        }
        None
    }
    pub fn recv_timeout(&mut self, timeout: Duration) -> Result<SinkRead, mpsc::RecvTimeoutError> {
        if self.reported_drained {
            return Err(mpsc::RecvTimeoutError::Disconnected);
        }
        if let Some(notice) = self.pending_notice() {
            return Ok(notice);
        }
        if let Some(packet) = self.pending.take() {
            return Ok(SinkRead::Packet(packet));
        }
        match self.receiver.recv_timeout(timeout) {
            Ok(packet) => {
                self.pending = Some(packet);
                if let Some(notice) = self.pending_notice() {
                    return Ok(notice);
                }
                Ok(SinkRead::Packet(
                    self.pending.take().expect("packet just received"),
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending_notice().ok_or(mpsc::RecvTimeoutError::Timeout)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(notice) = self.pending_notice() {
                    return Ok(notice);
                }
                let Some(summary) = self.shared.summary.get().cloned() else {
                    return Err(mpsc::RecvTimeoutError::Disconnected);
                };
                self.reported_drained = true;
                Ok(SinkRead::Drained(summary))
            }
        }
    }
    pub fn clock(&self) -> Option<&StreamClock> {
        self.shared.clock.get()
    }
    pub fn ready_observed_hns(&self) -> Option<u64> {
        self.shared.ready.get().copied()
    }
    pub fn prebarrier_witness(&self) -> Option<&BarrierWitness> {
        self.shared.witness.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const N: u64 = 10_000_000;
    const B: u64 = N + 10_000;
    const C: u64 = 48;
    fn scope(barrier: u64) -> SourcePacketScope {
        SourcePacketScope::new(Uuid::new_v4(), 1, "opaque-A".into(), 48000, barrier).unwrap()
    }
    fn stamp(first: u64, idle: Option<u64>, observed: u64) -> PacketStamp {
        PacketStamp {
            first_sample: first,
            silence_before: idle,
            native_index: first + 777,
            native_qpc_hns: N + first * 10_000_000 / 48000,
            observed_qpc_hns: observed,
            native_flags: 0,
        }
    }
    fn recv(rx: &mut EndpointPacketReceiver) -> SinkRead {
        rx.recv_timeout(Duration::from_millis(100)).unwrap()
    }
    fn bound(cap: usize) -> (EndpointPacketProducer, EndpointPacketReceiver) {
        let (mut tx, rx) = packet_channel(scope(B), cap).unwrap();
        tx.bind_native_clock(N, 48000).unwrap();
        tx.confirm_native_ready(N + 1000).unwrap();
        (tx, rx)
    }
    fn seeded(cap: usize) -> (EndpointPacketProducer, EndpointPacketReceiver) {
        let (mut tx, rx) = bound(cap);
        tx.push_native(stamp(0, None, B - 1), vec![7; C as usize])
            .unwrap();
        (tx, rx)
    }
    #[test]
    fn registration_projection_keeps_integer_epoch_and_no_implicit_origin() {
        let id = Uuid::new_v4();
        assert!(SourcePacketScope::new(id, 0, "A".into(), 48000, B).is_err());
        assert!(SourcePacketScope::new(Uuid::nil(), 1, "A".into(), 48000, B).is_err());
        assert!(SourcePacketScope::new(id, 1, "bad\0id".into(), 48000, B).is_err());
        assert!(SourcePacketScope::new(id, 1, "A".into(), 0, B).is_err());
        assert!(SourcePacketScope::new(id, 1, "A".into(), 48000, 0).is_err());
        assert_eq!(scope(B).epoch(), 1);
    }
    #[test]
    fn preroll_crossing_packet_keeps_suffix_and_original_native_provenance() {
        let (mut tx, mut rx) = bound(2);
        tx.push_native(stamp(0, None, N + 5000), vec![1; 24])
            .unwrap();
        let pcm = (24..72).map(|x| x as i16).collect();
        tx.push_native(stamp(24, None, B + 5000), pcm).unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 0, end: 24 });
        assert_eq!(p.samples, (48..72).map(|x| x as i16).collect::<Vec<_>>());
        assert_eq!(p.original_native_frames, FrameSpan { start: 24, end: 72 });
        assert_eq!(p.preroll_clipped_frames, 24);
        assert_eq!(p.native_stamp.first_sample, 24);
        assert_eq!(p.native_stamp.native_index, 801);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert!(!s.quarantined);
        assert_eq!(s.preroll_packets_dropped, 1);
        assert_eq!(s.preroll_frames_dropped, 48);
    }
    #[test]
    fn wholly_preroll_packets_never_become_visible_writer_pcm() {
        let (mut tx, mut rx) = bound(1);
        tx.push_native(stamp(0, None, N + 5000), vec![9; 24])
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        tx.push_native(stamp(24, None, B - 1), vec![8; 24]).unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        tx.push_native(stamp(C, None, B + 500), vec![3]).unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 0, end: 1 });
        assert_eq!(p.samples, vec![3]);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(s.preroll_packets_dropped, 2);
        assert_eq!(s.preroll_frames_dropped, 48);
        assert_eq!(s.accepted_packets, 1);
    }
    #[test]
    fn ceil_phase_is_exact_rational_metadata_below_one_sample() {
        let barrier = N + 101;
        let (mut tx, mut rx) = packet_channel(scope(barrier), 1).unwrap();
        tx.bind_native_clock(N, 48000).unwrap();
        tx.confirm_native_ready(N + 1).unwrap();
        assert_eq!(rx.clock().unwrap().cut_frame, 1);
        assert_eq!(rx.clock().unwrap().phase_numerator, 5_152_000);
        tx.push_native(stamp(0, None, N + 100), vec![0, 1, 2])
            .unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.samples, vec![1, 2]);
        assert_eq!(p.preroll_clipped_frames, 1);
        assert_eq!(p.frames, FrameSpan { start: 0, end: 2 });
        assert!(rx.clock().unwrap().phase_numerator < 10_000_000);
    }
    #[test]
    fn a_late_native_start_fails_instead_of_silently_rebasing_or_padding() {
        let (mut tx, mut rx) = packet_channel(scope(B), 1).unwrap();
        assert!(tx.bind_native_clock(B + 1, 48000).is_err());
        tx.finish(Ok(()));
        assert!(matches!(recv(&mut rx), SinkRead::StartupMissing { .. }));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::Protocol(_),
                ..
            }
        ));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn post_start_ready_confirmation_must_meet_the_shared_deadline() {
        let (mut tx, mut rx) = packet_channel(scope(B), 1).unwrap();
        tx.bind_native_clock(N, 48000).unwrap();
        assert!(tx.confirm_native_ready(B + 1).is_err());
        tx.finish(Ok(()));
        assert!(matches!(recv(&mut rx), SinkRead::StartupUnverified { .. }));
        assert!(matches!(recv(&mut rx), SinkRead::Fault { .. }));
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(s.ready_observed_hns, Some(B + 1));
        assert!(s.quarantined);
    }
    #[test]
    fn late_first_observation_with_complete_crossing_pcm_is_unverified_not_known_missing() {
        let (mut tx, mut rx) = bound(1);
        tx.push_native(stamp(0, Some(0), B + 10000), vec![6; 96])
            .unwrap();
        tx.finish(Ok(()));
        assert!(matches!(recv(&mut rx), SinkRead::StartupUnverified { .. }));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 0, end: 48 });
        assert_eq!(p.samples.len(), 48);
        assert_eq!(p.missing_before, None);
        assert_eq!(p.known_idle_before, None);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert!(s.quarantined);
        assert_eq!(s.prebarrier_witness, None);
    }
    #[test]
    fn first_postbarrier_packet_cannot_promote_legacy_some_zero_to_visible_silence() {
        let (mut tx, mut rx) = bound(1);
        tx.push_native(stamp(C + 10, Some(0), B + 10000), vec![6; 12])
            .unwrap();
        tx.finish(Ok(()));
        assert!(matches!(recv(&mut rx), SinkRead::StartupUnverified { .. }));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 10, end: 22 });
        assert_eq!(p.known_idle_before, None);
        assert_eq!(p.missing_before, None);
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn genuine_postbarrier_gap_stays_at_nonzero_frame_and_quarantines() {
        let (mut tx, mut rx) = seeded(1);
        tx.push_native(stamp(C + 10, None, B + 10000), vec![4; 12])
            .unwrap();
        tx.finish(Ok(()));
        let SinkRead::NativeGap { gap, .. } = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(
            gap.native_frames,
            FrameSpan {
                start: C,
                end: C + 10
            }
        );
        assert_eq!(gap.visible_frames, Some(FrameSpan { start: 0, end: 10 }));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 10, end: 22 });
        assert_eq!(p.missing_before, Some(FrameSpan { start: 0, end: 10 }));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn prebarrier_native_loss_remains_an_explicit_failure_diagnostic() {
        let (mut tx, mut rx) = bound(1);
        tx.push_native(stamp(8, None, N + 5000), vec![1; 16])
            .unwrap();
        tx.push_native(stamp(24, None, B + 5000), vec![2; 48])
            .unwrap();
        tx.finish(Ok(()));
        let SinkRead::NativeGap { gap, .. } = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(gap.native_frames, FrameSpan { start: 0, end: 8 });
        assert_eq!(gap.visible_frames, None);
        assert!(matches!(recv(&mut rx), SinkRead::Packet(_)));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn verified_idle_is_clipped_to_visible_interval_without_synthesizing_pcm() {
        let (mut tx, mut rx) = seeded(1);
        tx.push_native(stamp(C + 10, Some(C), B + 10000), vec![5; 12])
            .unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 10, end: 22 });
        assert_eq!(p.known_idle_before, Some(FrameSpan { start: 0, end: 10 }));
        assert_eq!(p.samples, vec![5; 12]);
        assert_eq!(p.missing_before, None);
    }
    #[test]
    fn prebarrier_idle_does_not_become_visible_origin_padding() {
        let (mut tx, mut rx) = bound(1);
        tx.push_native(stamp(24, Some(0), B - 1), vec![9; 48])
            .unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 0, end: 24 });
        assert_eq!(p.known_idle_before, None);
        assert_eq!(p.missing_before, None);
    }
    #[test]
    fn bounded_overflow_reports_visible_dropped_frames_and_drains_accepted_tail() {
        let (mut tx, mut rx) = seeded(1);
        tx.push_native(stamp(C, None, B + 1000), vec![1; 480])
            .unwrap();
        assert_eq!(
            tx.push_native(stamp(C + 480, None, B + 100000), vec![2; 480]),
            Err(SinkFault::QueueFull {
                dropped: FrameSpan {
                    start: 480,
                    end: 960
                }
            })
        );
        tx.finish(Ok(()));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::QueueFull { .. },
                ..
            }
        ));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.samples, vec![1; 480]);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(s.last_accepted_end_frame, Some(480));
        assert!(s.quarantined);
    }
    #[test]
    fn consumer_loss_and_abandoned_producer_never_claim_clean_completion() {
        let (mut tx, rx) = seeded(1);
        drop(rx);
        assert!(matches!(
            tx.push_native(stamp(C, None, B + 1000), vec![3]),
            Err(SinkFault::ConsumerGone {
                dropped: FrameSpan { start: 0, end: 1 }
            })
        ));
        let (mut tx, mut rx) = seeded(1);
        tx.push_native(stamp(C, None, B + 1000), vec![4]).unwrap();
        drop(tx);
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::AbandonedProducer,
                ..
            }
        ));
        assert!(matches!(recv(&mut rx), SinkRead::Packet(_)));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn no_visible_samples_after_only_preroll_are_not_clean_recording_output() {
        let (tx, mut rx) = seeded(1);
        tx.finish(Ok(()));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::Protocol(_),
                ..
            }
        ));
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(s.accepted_packets, 0);
        assert_eq!(s.preroll_frames_dropped, 48);
        assert!(s.quarantined);
    }
    #[test]
    fn clock_ready_and_packet_bounds_are_enforced_without_rate_conversion() {
        let (mut tx, _) = packet_channel(scope(B), 1).unwrap();
        assert!(tx.bind_native_clock(N, 44100).is_err());
        let (mut tx, _) = packet_channel(scope(B), 1).unwrap();
        tx.bind_native_clock(N, 48000).unwrap();
        assert!(tx.push_native(stamp(0, None, B - 1), vec![0]).is_err());
        let (mut tx, _) = bound(1);
        assert!(tx.confirm_native_ready(N + 2).is_err());
        let (mut tx, _) = seeded(1);
        assert!(tx
            .push_native(stamp(C, None, B + 1000), vec![1; 96001])
            .is_err());
        let (mut tx, _) = seeded(1);
        assert!(tx
            .push_native(stamp(C - 1, None, B + 1000), vec![1; 2])
            .is_err());
        let (mut tx, _) = seeded(1);
        assert!(tx
            .push_native(stamp(C, Some(C + 1), B + 1000), vec![1])
            .is_err());
    }
    #[test]
    fn accepted_one_frame_tail_survives_finish_and_keeps_native_stamp() {
        let (mut tx, mut rx) = seeded(1);
        let mut st = stamp(C, None, B + 1000);
        st.native_flags = 2;
        tx.push_native(st, vec![0]).unwrap();
        tx.finish(Ok(()));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.frames, FrameSpan { start: 0, end: 1 });
        assert_eq!(p.samples, vec![0]);
        assert_eq!(p.native_stamp.native_flags, 2);
        assert_eq!(
            p.original_native_frames,
            FrameSpan {
                start: C,
                end: C + 1
            }
        );
        assert_eq!(p.preroll_clipped_frames, 0);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert!(!s.quarantined);
        assert_eq!(s.ready_observed_hns, Some(N + 1000));
        assert!(s.prebarrier_witness.is_some());
        assert!(matches!(
            rx.recv_timeout(Duration::ZERO),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
    #[test]
    fn native_error_preserves_reason_and_private_queue_before_final_summary() {
        let (mut tx, mut rx) = seeded(1);
        tx.push_native(stamp(C, None, B + 1000), vec![8; 12])
            .unwrap();
        tx.finish(Err("device lost".into()));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::Native(_),
                ..
            }
        ));
        assert!(matches!(recv(&mut rx), SinkRead::Packet(_)));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn excessive_reserved_capacity_cannot_bypass_queue_memory_bound() {
        let (mut tx, mut rx) = seeded(1);
        let mut samples = Vec::with_capacity(96_001);
        samples.push(7);
        assert!(matches!(
            tx.push_native(stamp(C, None, B + 1000), samples),
            Err(SinkFault::Protocol(_))
        ));
        assert!(matches!(recv(&mut rx), SinkRead::Fault { .. }));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                accepted_packets: 0,
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn negative_first_native_qpc_anchor_is_unverified_without_invented_loss() {
        let (mut tx, mut rx) = bound(1);
        let mut st = stamp(0, Some(0), B - 1);
        st.native_qpc_hns = N - 1000;
        st.native_index = 12345;
        tx.push_native(st.clone(), (0..96).collect()).unwrap();
        tx.finish(Ok(()));
        let SinkRead::StartupUnverified { unverified, .. } = recv(&mut rx) else {
            panic!("clamped negative anchor must quarantine");
        };
        assert!(unverified.reason.contains("predates native origin"));
        assert_eq!(unverified.first_observed_hns, Some(B - 1));
        let SinkRead::Packet(p) = recv(&mut rx) else {
            panic!()
        };
        assert_eq!(p.native_stamp, st);
        assert_eq!(p.original_native_frames, FrameSpan { start: 0, end: 96 });
        assert_eq!(p.preroll_clipped_frames, 48);
        assert_eq!(p.frames, FrameSpan { start: 0, end: 48 });
        assert_eq!(p.samples, (48..96).collect::<Vec<i16>>());
        assert_eq!(p.missing_before, None);
        assert_eq!(p.known_idle_before, None);
        let SinkRead::Drained(s) = recv(&mut rx) else {
            panic!()
        };
        assert!(s.quarantined);
        assert_eq!(s.prebarrier_witness, None);
    }
    #[test]
    fn native_setup_reason_regression() {
        let (tx, mut rx) = packet_channel(scope(B), 1).unwrap();
        tx.finish(Err("Initialize HRESULT 0x88890004".into()));
        assert!(
            matches!(recv(&mut rx),SinkRead::Fault{fault:SinkFault::Native(ref reason),..}if reason=="Initialize HRESULT 0x88890004")
        );
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                clock: None,
                quarantined: true,
                ..
            })
        ));
    }
    #[test]
    fn rejected_crossing_preroll_counter_regression() {
        let (mut tx, rx) = bound(1);
        tx.push_native(stamp(0, None, N + 5000), vec![1; 24])
            .unwrap();
        let observer = tx.shared.clone();
        drop(rx);
        assert!(matches!(
            tx.push_native(stamp(24, None, B + 5000), vec![2; 48]),
            Err(SinkFault::ConsumerGone {
                dropped: FrameSpan { start: 0, end: 24 }
            })
        ));
        let summary = observer.summary.get().unwrap();
        assert_eq!(summary.preroll_frames_dropped, 48);
        assert_eq!(summary.accepted_packets, 0);
        assert!(summary.quarantined);
    }
    #[test]
    fn no_clock_finish_and_capacity_violations_are_explicit() {
        assert!(packet_channel(scope(B), 0).is_err());
        assert!(packet_channel(scope(B), MAX_PACKET_QUEUE_CAPACITY + 1).is_err());
        let (tx, mut rx) = packet_channel(scope(B), 1).unwrap();
        tx.finish(Ok(()));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Fault {
                fault: SinkFault::Protocol(_),
                ..
            }
        ));
        assert!(matches!(
            recv(&mut rx),
            SinkRead::Drained(StreamSummary {
                quarantined: true,
                ..
            })
        ));
    }
}
