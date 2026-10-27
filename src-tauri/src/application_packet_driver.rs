//! Private process-loopback lifecycle driver. Native calls remain on its caller.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, PartialEq)]
pub(super) enum PacketWait {
    Signaled,
    Timeout,
}

pub(super) trait PacketSource {
    type Packet;
    fn cancelled(&self) -> bool;
    fn start(&mut self) -> Result<(), String>;
    fn ready(&mut self) -> Result<(), String>;
    fn stop(&mut self) -> Result<(), String>;
    fn wait(&mut self) -> Result<PacketWait, String>;
    fn next_packet_size(&mut self) -> Result<u32, String>;
    fn read(&mut self) -> Result<Option<Self::Packet>, String>;
    fn accept(&mut self, packet: Self::Packet) -> Result<(), String>;
}

// Both errors carry evidence: a cleanup failure must not hide the first cause.
pub(super) fn combine(
    result: Result<(), String>,
    cleanup: Result<(), String>,
) -> Result<(), String> {
    match (result, cleanup) {
        (Err(primary), Err(cleanup)) => Err(format!("{primary}; cleanup: {cleanup}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}

fn catch_failure(run: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(run)).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic");
        Err(format!("application packet worker panicked: {message}"))
    })
}

fn stop_once<S: PacketSource>(source: &mut S, stopped: &mut bool) -> Result<(), String> {
    // Set before the call, including native failure or unwind; never retry Stop.
    *stopped = true;
    source.stop()
}

pub(super) fn run<S: PacketSource>(source: &mut S) -> Result<(), String> {
    let mut started = false;
    let mut stopped = false;
    let result = catch_failure(|| {
        if source.cancelled() {
            return Err("application capture cancelled before Start".into());
        }
        source.start()?;
        started = true;
        if source.cancelled() {
            return Err("application capture cancelled before readiness".into());
        }
        source.ready()?;
        loop {
            if source.cancelled() {
                stop_once(source, &mut stopped)?;
            } else if matches!(source.wait()?, PacketWait::Timeout) {
                continue;
            }
            loop {
                // A continuously nonempty buffer must not starve control.
                if !stopped && source.cancelled() {
                    stop_once(source, &mut stopped)?;
                }
                if source.next_packet_size()? == 0 {
                    break;
                }
                let Some(packet) = source.read()? else {
                    // Positive peek followed by zero frames is anomalous. While
                    // running, return to wait/control rather than hot retrying.
                    // After Stop, this cannot prove a complete final drain.
                    if stopped {
                        return Err(
                            "application stopped-buffer anomaly: positive peek, empty acquisition"
                                .into(),
                        );
                    }
                    break;
                };
                source.accept(packet)?;
            }
            if stopped {
                return Ok(());
            }
        }
    });
    let cleanup = if started && !stopped {
        catch_failure(|| stop_once(source, &mut stopped))
    } else {
        Ok(())
    };
    combine(result, cleanup)
}

pub(super) fn receive_readiness(
    receiver: &mpsc::Receiver<Result<String, String>>,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<String, String> {
    match receiver.recv_timeout(timeout) {
        Ok(result) => result,
        Err(error) => {
            // Private latch outlives the caller. It does not interrupt a blocked
            // wasapi activation constructor, but prevents its eventual Start.
            cancelled.store(true, Ordering::SeqCst);
            Err(format!("เตรียมจับเสียงแอปไม่ทันเวลา: {error}"))
        }
    }
}

pub(super) fn send_readiness(
    sender: mpsc::Sender<Result<String, String>>,
    cancelled: &AtomicBool,
    name: &str,
) -> Result<(), String> {
    sender.send(Ok(name.to_string())).map_err(|_| {
        cancelled.store(true, Ordering::SeqCst);
        "ผู้รอเริ่มจับเสียงแอปยกเลิกแล้ว".to_string()
    })
}

pub(super) fn classify_wait(status: u32, last_error: u32) -> Result<PacketWait, String> {
    use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};
    match status {
        WAIT_OBJECT_0 => Ok(PacketWait::Signaled),
        WAIT_TIMEOUT => Ok(PacketWait::Timeout),
        WAIT_FAILED => Err(format!(
            "รอสัญญาณเสียงจากแอปไม่สำเร็จ: WAIT_FAILED Win32={last_error}"
        )),
        other => Err(format!("รอสัญญาณเสียงจากแอปได้สถานะที่ไม่คาดหมาย: {other:#x}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Script {
        cancelled: bool,
        stops: usize,
        reads: usize,
        waits: usize,
        accepted: Vec<u8>,
        continuous: bool,
        starts: usize,
        ready_calls: usize,
        abandoned: bool,
        empty: bool,
        panic_read: bool,
        read_failure: bool,
        stop_failure: bool,
        start_failure: bool,
        wait_failure: bool,
        peek_failure: bool,
        queue_failure: bool,
        first_wait_timeout: bool,
        cancel_during_start: bool,
        local_cancel: Option<std::sync::Arc<AtomicBool>>,
        actions: Vec<&'static str>,
        owner: Option<std::thread::ThreadId>,
    }

    impl PacketSource for Script {
        type Packet = u8;
        fn cancelled(&self) -> bool {
            self.cancelled
                || self
                    .local_cancel
                    .as_ref()
                    .is_some_and(|cancel| cancel.load(Ordering::SeqCst))
        }
        fn start(&mut self) -> Result<(), String> {
            self.starts += 1;
            self.owner = Some(std::thread::current().id());
            self.actions.push("start");
            if self.cancel_during_start {
                self.cancelled = true;
            }
            if self.start_failure {
                Err("Start failed".into())
            } else {
                Ok(())
            }
        }
        fn ready(&mut self) -> Result<(), String> {
            self.ready_calls += 1;
            self.actions.push("ready");
            if self.abandoned {
                Err("ready receiver abandoned".into())
            } else {
                Ok(())
            }
        }
        fn stop(&mut self) -> Result<(), String> {
            self.stops += 1;
            assert_eq!(self.owner, Some(std::thread::current().id()));
            self.actions.push("stop");
            if self.stop_failure {
                Err("native Stop failed".into())
            } else {
                Ok(())
            }
        }
        fn wait(&mut self) -> Result<PacketWait, String> {
            self.waits += 1;
            self.actions.push("wait");
            if self.wait_failure {
                return Err("wait failed".into());
            }
            if self.first_wait_timeout && self.waits == 1 {
                return Ok(PacketWait::Timeout);
            }
            if self.empty && self.waits == 2 {
                self.cancelled = true;
            }
            Ok(PacketWait::Signaled)
        }
        fn next_packet_size(&mut self) -> Result<u32, String> {
            self.actions.push("peek");
            if self.peek_failure && self.reads == 1 {
                return Err("peek failed".into());
            }
            // Old starvation fails at a bounded operation instead of hanging.
            if self.reads >= 4 {
                return Err("script operation bound: native Stop starved".into());
            }
            Ok(u32::from(if self.continuous && self.stops == 0 {
                true
            } else {
                self.reads < 2
            }))
        }
        fn read(&mut self) -> Result<Option<u8>, String> {
            self.reads += 1;
            self.actions.push("read");
            if self.panic_read && self.reads == 2 {
                panic!("script read panic");
            }
            if self.read_failure && self.reads == 2 {
                return Err("packet read failed".into());
            }
            if self.empty {
                return Ok(None);
            }
            Ok(Some(self.reads as u8))
        }
        fn accept(&mut self, packet: u8) -> Result<(), String> {
            self.actions.push("accept");
            if self.queue_failure && self.reads == 2 {
                return Err("queue full".into());
            }
            self.accepted.push(packet);
            if self.continuous || self.reads == 2 {
                self.cancelled = true;
            }
            Ok(())
        }
    }

    #[test]
    fn characterizes_finite_packets_and_final_stop() {
        let mut source = Script::default();
        run(&mut source).unwrap();
        assert_eq!(source.accepted, [1, 2]);
        assert_eq!(source.stops, 1);
        assert_eq!(source.waits, 1);
    }

    #[test]
    fn stop_interrupts_continuous_inner_drain_and_accepts_final_packet() {
        let mut source = Script {
            continuous: true,
            ..Script::default()
        };
        run(&mut source).unwrap();
        assert_eq!(source.accepted, [1, 2]);
        assert_eq!(source.stops, 1);
        assert_eq!(source.reads, 2);
        assert_eq!(
            source.actions,
            [
                "start", "ready", "wait", "peek", "read", "accept", "stop", "peek", "read",
                "accept", "peek"
            ]
        );
    }

    #[test]
    fn cancellation_before_start_never_starts_or_advertises_ready() {
        let mut source = Script {
            cancelled: true,
            ..Script::default()
        };
        assert!(run(&mut source).unwrap_err().contains("cancel"));
        assert_eq!((source.starts, source.ready_calls, source.stops), (0, 0, 0));
        assert!(source.accepted.is_empty());
    }

    #[test]
    fn abandoned_readiness_stops_without_waiting_or_reading() {
        let mut source = Script {
            abandoned: true,
            ..Script::default()
        };
        assert!(run(&mut source)
            .unwrap_err()
            .contains("ready receiver abandoned"));
        assert_eq!((source.starts, source.ready_calls, source.stops), (1, 1, 1));
        assert_eq!((source.waits, source.reads), (0, 0));
    }

    #[test]
    fn empty_after_positive_peek_yields_then_fails_honestly_when_stopped() {
        let mut source = Script {
            empty: true,
            ..Script::default()
        };
        let error = run(&mut source).unwrap_err();
        assert!(error.contains("stopped-buffer anomaly"), "{error}");
        assert_eq!((source.waits, source.reads, source.stops), (2, 2, 1));
        assert!(source.accepted.is_empty());
    }

    #[test]
    fn read_failure_preserves_cleanup_failure_and_accepted_packet() {
        let mut source = Script {
            read_failure: true,
            stop_failure: true,
            ..Script::default()
        };
        let error = run(&mut source).unwrap_err();
        assert!(error.contains("packet read failed"));
        assert!(error.contains("native Stop failed"), "{error}");
        assert_eq!(source.stops, 1);
        assert_eq!(source.accepted, [1]);
    }

    #[test]
    fn unwind_stops_once_and_preserves_panic_and_cleanup_evidence() {
        let mut source = Script {
            panic_read: true,
            stop_failure: true,
            ..Script::default()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut source)));
        assert!(
            result.is_ok(),
            "driver must clean up its native owner before writer join"
        );
        let error = result.unwrap().unwrap_err();
        assert!(error.contains("script read panic"));
        assert!(error.contains("native Stop failed"));
        assert_eq!(source.stops, 1);
        assert_eq!(source.accepted, [1]);
    }

    #[test]
    fn cancellation_after_start_stops_before_readiness() {
        let mut source = Script {
            cancel_during_start: true,
            ..Script::default()
        };
        assert!(run(&mut source).unwrap_err().contains("cancel"));
        assert_eq!(
            (
                source.starts,
                source.ready_calls,
                source.stops,
                source.reads
            ),
            (1, 0, 1, 0)
        );
    }

    #[test]
    fn timeout_latch_survives_abandoned_caller_without_changing_external_stop() {
        let external_stop = AtomicBool::new(false);
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let worker_cancel = std::sync::Arc::clone(&cancelled);
        let (_sender, receiver) = mpsc::channel();
        assert!(receive_readiness(&receiver, &cancelled, Duration::ZERO).is_err());
        drop(cancelled);
        drop(receiver);
        let mut source = Script {
            local_cancel: Some(worker_cancel),
            ..Script::default()
        };
        assert!(run(&mut source).unwrap_err().contains("cancel"));
        assert_eq!((source.starts, source.ready_calls, source.stops), (0, 0, 0));
        assert!(!external_stop.load(Ordering::SeqCst));
    }

    #[test]
    fn disconnected_readiness_latches_cancel_instead_of_fake_success() {
        let (sender, receiver) = mpsc::channel();
        drop(receiver);
        let cancelled = AtomicBool::new(false);
        assert!(send_readiness(sender, &cancelled, "selected PID").is_err());
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[test]
    fn readiness_success_and_worker_error_are_delivered_exactly() {
        let (sender, receiver) = mpsc::channel();
        let cancelled = AtomicBool::new(false);
        send_readiness(sender, &cancelled, "selected PID").unwrap();
        assert_eq!(
            receive_readiness(&receiver, &cancelled, Duration::ZERO).unwrap(),
            "selected PID"
        );
        assert!(!cancelled.load(Ordering::SeqCst));
        let (sender, receiver) = mpsc::channel();
        sender.send(Err("activation failed".into())).unwrap();
        assert_eq!(
            receive_readiness(&receiver, &cancelled, Duration::ZERO).unwrap_err(),
            "activation failed"
        );
    }

    #[test]
    fn native_wait_status_keeps_timeout_failure_and_unexpected_distinct() {
        use windows_sys::Win32::Foundation::{
            WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
        };
        assert_eq!(
            classify_wait(WAIT_OBJECT_0, 0).unwrap(),
            PacketWait::Signaled
        );
        assert_eq!(classify_wait(WAIT_TIMEOUT, 0).unwrap(), PacketWait::Timeout);
        let error = classify_wait(WAIT_FAILED, 6).unwrap_err();
        assert!(error.contains("WAIT_FAILED") && error.contains("Win32=6"));
        assert!(classify_wait(WAIT_ABANDONED, 0).is_err());
    }

    #[test]
    fn timeout_returns_to_control_without_draining_packets() {
        let mut source = Script {
            first_wait_timeout: true,
            ..Script::default()
        };
        run(&mut source).unwrap();
        assert_eq!(
            &source.actions[..5],
            ["start", "ready", "wait", "wait", "peek"]
        );
        assert_eq!(source.accepted, [1, 2]);
        assert_eq!(source.stops, 1);
    }

    #[test]
    fn wait_peek_queue_and_start_errors_stop_only_started_owner_once() {
        for (point, expected, accepted, stops) in [
            ("wait", "wait failed", vec![], 1),
            ("peek", "peek failed", vec![1], 1),
            ("queue", "queue full", vec![1], 1),
            ("start", "Start failed", vec![], 0),
            ("stop", "native Stop failed", vec![1, 2], 1),
        ] {
            let mut source = Script {
                wait_failure: point == "wait",
                peek_failure: point == "peek",
                queue_failure: point == "queue",
                start_failure: point == "start",
                stop_failure: point == "stop",
                ..Script::default()
            };
            assert!(run(&mut source).unwrap_err().contains(expected), "{point}");
            assert_eq!(source.accepted, accepted, "{point}");
            assert_eq!(source.stops, stops, "{point}");
        }
    }

    #[test]
    fn writer_failure_is_retained_with_primary_and_stop_errors() {
        assert_eq!(
            combine(
                combine(Err("read cause".into()), Err("Stop failed".into())),
                Err("writer failed".into())
            )
            .unwrap_err(),
            "read cause; cleanup: Stop failed; cleanup: writer failed"
        );
    }
}
