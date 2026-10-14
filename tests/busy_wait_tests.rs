#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! BUSY wait failure modes (backlog 2g): a stuck pin is an error rather than a silent `Ok`, and a
//! failing BUSY pin is an error in async mode as it already was in blocking mode.
//!
//! Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

use embedded_hal::digital::{ErrorKind, ErrorType as DigitalErrorType, InputPin};
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use epdsi::prelude::*;

mod support;
use support::*;

/// A BUSY pin whose every read fails.
struct FailingBusy;

impl DigitalErrorType for FailingBusy {
    type Error = ErrorKind;
}

impl InputPin for FailingBusy {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Err(ErrorKind::Other)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Err(ErrorKind::Other)
    }
}

#[cfg(not(feature = "blocking"))]
impl Wait for FailingBusy {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        Err(ErrorKind::Other)
    }
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        Err(ErrorKind::Other)
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        Err(ErrorKind::Other)
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        Err(ErrorKind::Other)
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        Err(ErrorKind::Other)
    }
}

/// BUSY stuck at its asserted level for the whole 60 s bound.
///
/// Blocking only: the async bus waits on the pin's edge and has no bound to hit. See the busy-wait
/// block doc in `src/bus.rs`.
#[cfg(feature = "blocking")]
#[test]
fn test_wait_busy_with_delay_times_out_on_stuck_busy() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    // Active-high panel with BUSY held high forever.
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut delay = RecordingDelay::new();

    let result = bus.wait_busy_with_delay(&mut delay, true);

    assert_eq!(result, Err(EpdBusError::Timeout));
    assert_eq!(
        delay.calls_ms.len(),
        60_001,
        "bound should be 60,000 ms of 1 ms polls plus the one that trips it"
    );
}

/// An idle pin still returns `Ok` without polling: the new error must not fire on a healthy panel.
#[cfg(feature = "blocking")]
#[test]
fn test_wait_busy_with_delay_ok_when_idle() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(false));
    let mut delay = RecordingDelay::new();

    bus.wait_busy_with_delay(&mut delay, true).unwrap();
    assert!(delay.calls_ms.is_empty(), "polled an idle pin");
}

/// A BUSY pin that errors surfaces as `EpdBusError::Busy` from every wait, in both modes. Async
/// mode used to drop this error and return `Ok`.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn failing_busy_pin_is_reported_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FailingBusy);
    let mut delay = DummyDelay;

    assert_eq!(
        bus.wait_busy(true).await,
        Err(EpdBusError::Busy(ErrorKind::Other))
    );
    assert_eq!(
        bus.wait_busy_with_delay(&mut delay, true).await,
        Err(EpdBusError::Busy(ErrorKind::Other))
    );
    assert_eq!(
        bus.wait_busy_assert(&mut delay, true, 5).await,
        Err(EpdBusError::Busy(ErrorKind::Other))
    );
}
epd_test!(
    test_failing_busy_pin_is_reported,
    failing_busy_pin_is_reported_body
);
