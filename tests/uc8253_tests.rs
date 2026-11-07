#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

#[cfg(feature = "blocking")]
use core::cell::RefCell;
use epdsi::prelude::*;

mod support;
use support::*;

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin};

#[test]
fn test_uc8253_gdey037t03_panel_dimensions() {
    assert_eq!(GDEY037T03::WIDTH, 240);
    assert_eq!(GDEY037T03::HEIGHT, 416);
    assert_eq!(GxEPD2_370_GDEY037T03::WIDTH, 240);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
}
epd_test!(test_uc8253_init_sequence, uc8253_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_set_window_byte_alignment_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);

    // set_window only records the area; the UC8253 needs the window re-opened around every
    // operation, so no commands are emitted here.
    // x_start=10 (rounds down to 8 via &0xFFF8), x_end=20 (rounds up to 23 via |0x0007)
    controller.set_window(&mut bus, 10, 5, 20, 15).await.unwrap();
    assert!(bus_backend.records.borrow().is_empty());

    // The recorded area is emitted around the next RAM write, and closed again afterwards.
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91), // PARTIAL_IN
            SpiRecord::Command(0x90), // PARTIAL_WINDOW
            SpiRecord::Data(vec![8, 23, 0x00, 0x05, 0x00, 0x0F, 0x01]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xAA]),
            SpiRecord::Command(0x92), // PARTIAL_OUT
        ]
    );

    // clear_window returns to full-frame addressing: no window commands at all.
    bus_backend.records.borrow_mut().clear();
    controller.clear_window();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xBB])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xBB])]
    );
}
epd_test!(
    test_uc8253_set_window_byte_alignment,
    uc8253_set_window_byte_alignment_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_write_frame_channel_routing_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xCC])]
    );
}
epd_test!(
    test_uc8253_write_frame_channel_routing,
    uc8253_write_frame_channel_routing_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_trigger_refresh_all_modes_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full (default), no window set: no PARTIAL_OUT, CDI=0x97, no CCSET/TSSET.
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
        ]
    );

    // FastFull: CCSET/TSSET(0x5A) before CDI=0x97, panel-setting reset after PowerOff.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastFull);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE5),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );

    // Partial with a window set: the refresh gets its own PARTIAL_IN/PARTIAL_WINDOW session,
    // closed by PARTIAL_OUT only after DISPLAY_REFRESH completes.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::Partial);
    controller.set_window(&mut bus, 0, 0, 239, 415).await.unwrap();
    bus_backend.records.borrow_mut().clear();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91), // PARTIAL_IN, re-opened for the refresh
            SpiRecord::Command(0x90),
            SpiRecord::Data(vec![0, 239, 0x00, 0x00, 0x01, 0x9F, 0x01]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x92), // PARTIAL_OUT, after the refresh
        ]
    );

    // FastPartial: CCSET/TSSET(0x6E) before CDI=0xD7.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastPartial);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE5),
            SpiRecord::Data(vec![0x6E]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
}
epd_test!(
    test_uc8253_trigger_refresh_all_modes,
    uc8253_trigger_refresh_all_modes_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}
epd_test!(test_uc8253_sleep, uc8253_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_busy_low_polarity_smoke_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle.
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY037T03>::new(controller).build(bus);

    driver.init(&mut delay).await.expect("UC8253 init failed");
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("UC8253 clear frame failed");
    driver.refresh(&mut delay).await.expect("UC8253 refresh failed");
    driver.sleep(&mut delay).await.expect("UC8253 sleep failed");
}
epd_test!(
    test_uc8253_busy_low_polarity_smoke,
    uc8253_busy_low_polarity_smoke_body
);

#[test]
fn test_uc8253_se0352n14_panel_dimensions() {
    // 240 x 360 is the raster, not Waveshare's advertised 360 x 240 viewing orientation:
    // 30 bytes per line over 360 lines.
    assert_eq!(SE0352N14TNGA0::WIDTH, 240);
    assert_eq!(SE0352N14TNGA0::HEIGHT, 360);
    assert_eq!(SE0352N14TNGA0::COLOR_MODE, ColorMode::TriColor);
}

/// Byte-for-byte parity with Waveshare's `EPD_3IN52B_Init()` and with the `ws_3in52b_init_code[]`
/// list in the Adafruit_EPD port. Also pins the RESOLUTION bytes derived from the controller's
/// configured dimensions to Waveshare's literal `[0xF0, 0x01, 0x68]`.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_se0352n14_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x04), // POWER_ON
            SpiRecord::Command(0x50), // CDI - 0x87, NOT the GDEY037T03 profile's 0x97
            SpiRecord::Data(vec![0x87]),
            SpiRecord::Command(0x00), // PANEL_SETTING
            SpiRecord::Data(vec![0x03, 0x0D]),
            SpiRecord::Command(0x61), // RESOLUTION - 240 x 0x0168 (360)
            SpiRecord::Data(vec![0xF0, 0x01, 0x68]),
            SpiRecord::Command(0x06), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0x2F, 0x2F, 0x2E]),
        ]
    );
}
epd_test!(
    test_uc8253_se0352n14_init_sequence,
    uc8253_se0352n14_init_sequence_body
);

/// The two variants put the Black/White plane on opposite RAM commands. Crossing them routes
/// black pixels into the red plane, so both directions are asserted together.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_se0352n14_plane_routing_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10), // WRITE_OLD_DATA carries Black/White here
            SpiRecord::Data(vec![0xAA, 0xBB]),
        ]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x13), // WRITE_NEW_DATA carries Red here
            SpiRecord::Data(vec![0xCC]),
        ]
    );

    // write_frame_pattern must route identically to write_frame.
    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x00, 2)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x00, 0x00])]
    );

    // The default variant is untouched: Black/White stays on WRITE_NEW_DATA.
    bus_backend.records.borrow_mut().clear();
    let mut default_variant = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    assert_eq!(default_variant.variant(), Uc8253Variant::Gdey037t03);
    default_variant
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xAA])]
    );
}
epd_test!(
    test_uc8253_se0352n14_plane_routing,
    uc8253_se0352n14_plane_routing_body
);

/// Refresh on this panel is a bare `DISPLAY_REFRESH`. Re-issuing `CDI` would move the DDX
/// polarity bits away from the `0x87` set at init and invert black and white, and the panel is
/// already powered from `init_sequence` — so neither may leak in, not even under a fast mode,
/// which this panel does not support at all.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_se0352n14_trigger_refresh_ignores_refresh_mode_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    for mode in [
        Uc8253RefreshMode::Full,
        Uc8253RefreshMode::FastFull,
        Uc8253RefreshMode::Partial,
        Uc8253RefreshMode::FastPartial,
    ] {
        bus_backend.records.borrow_mut().clear();
        let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
            .with_variant(Uc8253Variant::Se0352n14)
            .with_refresh_mode(mode);
        controller
            .trigger_refresh(&mut bus, &mut delay)
            .await
            .unwrap();

        assert_eq!(
            bus_backend.records.borrow().clone(),
            vec![
                SpiRecord::Command(0x04), // POWER_ON — the charge pump drops after each update
                SpiRecord::Command(0x12), // DISPLAY_REFRESH
            ],
            "refresh mode {mode:?} leaked commands into the SE0352N14 refresh"
        );
    }
}
epd_test!(
    test_uc8253_se0352n14_trigger_refresh_ignores_refresh_mode,
    uc8253_se0352n14_trigger_refresh_ignores_refresh_mode_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_se0352n14_driver_smoke_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle.
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let mut driver = EpdBuilder::<_, SE0352N14TNGA0>::new(controller).build(bus);

    assert_eq!(driver.width(), 240);
    assert_eq!(driver.height(), 360);

    driver.init(&mut delay).await.expect("SE0352N14 init failed");
    // 0x00 is white in BOTH planes on this panel, unlike the monochrome GDEY037T03's 0xFF.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0x00)
        .await
        .expect("SE0352N14 black/white clear failed");
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .await
        .expect("SE0352N14 red clear failed");
    driver
        .refresh(&mut delay)
        .await
        .expect("SE0352N14 refresh failed");
    driver.sleep(&mut delay).await.expect("SE0352N14 sleep failed");
}
epd_test!(
    test_uc8253_se0352n14_driver_smoke,
    uc8253_se0352n14_driver_smoke_body
);

struct CountingDelay(u32);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl DelayNs for CountingDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
    async fn delay_ms(&mut self, ms: u32) {
        self.0 += ms;
    }
}

/// A fixed settling delay after `DISPLAY_REFRESH` is not enough: BUSY assertion latency varies,
/// and a 10 ms guard was observed holding on some refreshes and missing on others on the same
/// XIAO ESP32-C3. Missing it leaves the panel a frame behind, because the caller writes into RAM
/// while the update is still running. `wait_busy_assert` waits for the edge instead of guessing.
///
/// Blocking-only: async's `wait_busy_assert` has no polling loop to observe a late assertion
/// against — it's a single `Wait::wait_for_high/low().await` with no timeout, by design (see
/// `src/bus.rs`'s async busy-wait block doc). The async counterpart below tests that instead.
#[cfg(feature = "blocking")]
#[test]
fn test_wait_busy_assert_waits_for_a_late_assertion() {
    /// BUSY that reads idle until `assert_after` polls have happened, then reads busy.
    ///
    /// Active-LOW, so "idle" is HIGH. Modelled on the real failure: the panel had not pulled BUSY
    /// down yet when the driver first looked.
    struct LateBusy {
        polls: RefCell<u32>,
        assert_after: u32,
    }

    impl DigitalErrorType for &LateBusy {
        type Error = core::convert::Infallible;
    }

    impl InputPin for &LateBusy {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            let mut polls = self.polls.borrow_mut();
            let now = *polls;
            *polls += 1;
            Ok(now < self.assert_after)
        }
        fn is_low(&mut self) -> Result<bool, Self::Error> {
            Ok(*self.polls.borrow() >= self.assert_after)
        }
    }

    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);

    // Asserts only on the 26th poll — past any 10 ms fixed guard.
    let busy = LateBusy {
        polls: RefCell::new(0),
        assert_after: 25,
    };
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, &busy);
    let mut delay = CountingDelay(0);

    let asserted = bus.wait_busy_assert(&mut delay, false, 500).unwrap();
    assert!(asserted, "gave up before BUSY asserted");
    assert!(
        delay.0 >= 25,
        "returned after {} ms, before the panel asserted at 25 ms",
        delay.0
    );
}

/// Async mode: `wait_busy_assert` always reports the edge as observed, immediately — there is no
/// polling loop and no timeout to race against without an executor (see decision in
/// `src/bus.rs`). `delay`/`timeout_ms` are accepted but unused, purely so call sites match the
/// blocking signature.
#[cfg(not(feature = "blocking"))]
#[test]
fn test_wait_busy_assert_waits_for_a_late_assertion() {
    pollster::block_on(async {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut delay = CountingDelay(0);

        let asserted = bus.wait_busy_assert(&mut delay, false, 500).await.unwrap();
        assert!(asserted, "async wait_busy_assert must always report true");
        assert_eq!(
            delay.0, 0,
            "async wait_busy_assert must not touch the delay at all"
        );
    });
}

/// A panel that never asserts must not hang the caller, so the wait is bounded and reports the
/// timeout rather than erroring — a missing panel still reads idle and falls straight through.
///
/// Blocking-only: see the note on `test_wait_busy_assert_waits_for_a_late_assertion` above for
/// why async has a different counterpart instead of a shared body.
#[cfg(feature = "blocking")]
#[test]
fn test_wait_busy_assert_times_out_on_a_silent_panel() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    // Reads HIGH forever: idle forever, for an active-LOW panel.
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut delay = CountingDelay(0);

    let asserted = bus.wait_busy_assert(&mut delay, false, 50).unwrap();
    assert!(!asserted, "claimed BUSY asserted on a silent panel");
    assert_eq!(delay.0, 50, "did not honour the timeout");
}

/// Async mode: a pin that is already at the target level resolves `wait_busy_assert` immediately
/// with `true` — there is no concept of "silent panel times out" without a polling loop.
#[cfg(not(feature = "blocking"))]
#[test]
fn test_wait_busy_assert_times_out_on_a_silent_panel() {
    pollster::block_on(async {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut delay = CountingDelay(0);

        let asserted = bus.wait_busy_assert(&mut delay, false, 50).await.unwrap();
        assert!(asserted, "async wait_busy_assert must always report true");
    });
}

/// A fast update must restore the panel setting with a settling gap between the soft reset
/// (`0x1E`, RST_N low) and its release (`0x1F`). Issuing them back-to-back lets the reset's
/// power-on defaults win, which flips the scan direction and rotates every later frame.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_fast_update_restores_panel_setting_with_settling_delay_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = CountingDelay(0);

    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastPartial);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    let tail = &records[records.len() - 4..];
    assert_eq!(
        tail,
        &[
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
    assert!(
        delay.0 >= 1,
        "no settling delay between the soft reset and its release"
    );
}
epd_test!(
    test_uc8253_fast_update_restores_panel_setting_with_settling_delay,
    uc8253_fast_update_restores_panel_setting_with_settling_delay_body
);

#[test]
fn test_uc8252c213_panel_dimensions() {
    // 122 x 250 is the raster, not the landscape 250 x 122 viewing orientation.
    assert_eq!(Uc8252c213::WIDTH, 122);
    assert_eq!(Uc8252c213::HEIGHT, 250);
    assert_eq!(Uc8252c213::COLOR_MODE, ColorMode::BlackWhite);
    // The all-caps spelling names the same panel.
    assert_eq!(UC8252C213::WIDTH, 122);
    assert_eq!(UC8252C213::HEIGHT, 250);
}

/// Splits the recorded SPI traffic into `(command, payload)` pairs, so the assertions can check
/// the command order and each payload without listing every byte of the frame buffers.
fn command_payloads(records: &[SpiRecord]) -> Vec<(u8, Vec<u8>)> {
    let mut out: Vec<(u8, Vec<u8>)> = Vec::new();
    for record in records {
        match record {
            SpiRecord::Command(c) => out.push((*c, Vec::new())),
            SpiRecord::Data(d) => {
                if let Some(last) = out.last_mut() {
                    last.1.extend_from_slice(d);
                }
            }
        }
    }
    out
}

/// The 2.13" `UC8252C213` profile runs its own register init, clears both RAM planes to white,
/// then loads the GC LUTs twice (ping-ponging LUT2/LUT3 order) with an Auto Sequence trigger
/// after each.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8252c213_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    // BUSY is not polled during init, so its level is irrelevant here.
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(Uc8252c213::WIDTH, Uc8252c213::HEIGHT)
        .with_variant(Uc8253Variant::Sfe213zhbw);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    let pairs = command_payloads(&records);
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        commands,
        vec![
            0x00, 0x01, 0x03, 0x06, 0x30, 0x60, 0x82, 0xE3, 0x41, 0x61, 0x65, // register init
            0x10, 0x13, // clear both planes
            0x20, 0x21, 0x24, 0x22, 0x23, 0x17, // LUT GC, then Auto Sequence
            0x20, 0x21, 0x24, 0x23, 0x22, 0x17, // LUT GC again, LUT2/LUT3 swapped
        ]
    );

    assert_eq!(pairs[0].1, vec![0xFF, 0x0A]);
    assert_eq!(pairs[1].1, vec![0x03, 0x00, 0x3F, 0x3F, 0x03]);
    assert_eq!(pairs[9].1, vec![0x80, 0x00, 0xFA]);
    assert_eq!(pairs[10].1, vec![0x00, 0x00, 0x00]);
    // 122 px rows padded to 16 bytes each, 250 rows: 4,000 bytes per plane, white (0xFF).
    assert_eq!(pairs[11].1, vec![0xFF; 4000]);
    assert_eq!(pairs[12].1, vec![0xFF; 4000]);
    for &index in &[13, 14, 15, 16, 17, 19, 20, 21, 22, 23] {
        assert_eq!(pairs[index].1.len(), 42, "LUT payload at index {index}");
    }
    assert_eq!(pairs[18].1, vec![0xA5]);
    assert_eq!(pairs[24].1, vec![0xA5]);
}
epd_test!(test_uc8252c213_init_sequence, uc8252c213_init_sequence_body);

/// `Uc8253Variant::Sfe213zhbw` refreshes by loading the DU LUTs and triggering the Auto
/// Sequence (`0x17`, payload `0xA5`). The LUT2/LUT3 order alternates on every refresh.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8252c213_refresh_lut_du_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle, so the BUSY waits return immediately.
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8253Controller::new(Uc8252c213::WIDTH, Uc8252c213::HEIGHT)
        .with_variant(Uc8253Variant::Sfe213zhbw);

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(commands, vec![0x20, 0x21, 0x24, 0x22, 0x23, 0x17]);
    for &index in &[0, 1, 2, 3, 4] {
        assert_eq!(pairs[index].1.len(), 42, "LUT payload at index {index}");
    }
    assert_eq!(pairs[5].1, vec![0xA5]);

    bus_backend.records.borrow_mut().clear();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(commands, vec![0x20, 0x21, 0x24, 0x23, 0x22, 0x17]);
}
epd_test!(
    test_uc8252c213_refresh_lut_du_sequence,
    uc8252c213_refresh_lut_du_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8252c213_driver_smoke_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle.
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(Uc8252c213::WIDTH, Uc8252c213::HEIGHT)
        .with_variant(Uc8253Variant::Sfe213zhbw);
    let mut driver = EpdBuilder::<_, Uc8252c213>::new(controller).build(bus);

    assert_eq!(driver.width(), 122);
    assert_eq!(driver.height(), 250);

    driver.init(&mut delay).await.expect("UC8252C213 init failed");
    // Monochrome UC8253 convention: 0xFF is white on the Black/White plane.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("UC8252C213 black/white clear failed");
    driver
        .refresh(&mut delay)
        .await
        .expect("UC8252C213 refresh failed");
    driver
        .sleep(&mut delay)
        .await
        .expect("UC8252C213 sleep failed");
}
epd_test!(test_uc8252c213_driver_smoke, uc8252c213_driver_smoke_body);

/// First 7 bytes (group 1) of a 42-byte `Sfe213zhbw` LUT, the rest asserted zero.
fn lut_group1(payload: &[u8]) -> [u8; 7] {
    assert_eq!(payload.len(), 42);
    assert!(payload[7..].iter().all(|&b| b == 0), "only group 1 is used");
    payload[..7].try_into().unwrap()
}

fn sfe213zhbw_bus_controller(
    bus_backend: &RecordingSpiBus,
) -> (
    SpiBusWrapper<&RecordingSpiBus, TestDc<'_>, DummyPin, FixedPin>,
    Uc8253Controller,
) {
    // Active-low BUSY: `true` (high) means idle, so the BUSY waits return immediately.
    let bus = SpiBusWrapper::new(bus_backend, TestDc(bus_backend), DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(Uc8252c213::WIDTH, Uc8252c213::HEIGHT)
        .with_variant(Uc8253Variant::Sfe213zhbw);
    (bus, controller)
}

/// `GlobalClear` loads the vendor GC LUTs (the init waveform) and triggers the Auto Sequence.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_global_clear_body() {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut controller = controller.with_refresh_mode(Uc8253RefreshMode::GlobalClear);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(commands, vec![0x20, 0x21, 0x24, 0x22, 0x23, 0x17]);
    assert_eq!(lut_group1(&pairs[0].1), [0x01, 0x0F, 0x0F, 0x0F, 0x01, 0x01, 0x01]);
    assert_eq!(lut_group1(&pairs[1].1), [0x01, 0x4F, 0x8F, 0x0F, 0x01, 0x01, 0x01]);
    assert_eq!(lut_group1(&pairs[2].1), [0x01, 0x0F, 0x8F, 0x4F, 0x01, 0x01, 0x01]);
    assert_eq!(lut_group1(&pairs[3].1), [0x01, 0x0F, 0x8F, 0x0F, 0x01, 0x01, 0x01]);
    assert_eq!(lut_group1(&pairs[4].1), [0x01, 0x4F, 0x8F, 0x4F, 0x01, 0x01, 0x01]);
    assert_eq!(pairs[5].1, vec![0xA5]);
}
epd_test!(test_sfe213zhbw_global_clear, sfe213zhbw_global_clear_body);

/// Balanced gray level table: `k` frames towards white (VSL), then `k` towards black (VSH).
fn balanced(k: u8) -> [u8; 7] {
    [0x01, 0x80 | k, 0x40 | k, 0x00, 0x00, 0x01, 0x01]
}

/// `Gray4` runs at 100 Hz and loads one balanced table per gray level. With the default 160 ms
/// (16 frames), Light/Dark/Black get 5/11/16 frames each way and White an empty table. Light sits
/// on WK (`0x23`), Dark on KW (`0x22`), Black on KK; the next load swaps `0x22`/`0x23` like
/// every other `Sfe213zhbw` LUT load.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_gray4_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut controller = controller.with_refresh_mode(Uc8253RefreshMode::Gray4);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        commands,
        vec![0x30, 0x20, 0x21, 0x24, 0x22, 0x23, 0x17, 0x30]
    );
    assert_eq!(pairs[0].1, vec![0x13]); // 100 Hz
    assert_eq!(lut_group1(&pairs[1].1), [0x01, 0x10, 0x10, 0x00, 0x00, 0x01, 0x01]);
    assert_eq!(pairs[2].1, vec![0x00; 42]); // White: untouched
    assert_eq!(lut_group1(&pairs[3].1), balanced(16)); // Black
    assert_eq!(lut_group1(&pairs[4].1), balanced(11)); // Dark
    assert_eq!(lut_group1(&pairs[5].1), balanced(5)); // Light
    assert_eq!(pairs[6].1, vec![0xA5]);
    assert_eq!(pairs[7].1, vec![0x09]); // back to 50 Hz

    bus_backend.records.borrow_mut().clear();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        commands,
        vec![0x30, 0x20, 0x21, 0x24, 0x23, 0x22, 0x17, 0x30]
    );
    assert_eq!(lut_group1(&pairs[4].1), balanced(11)); // Dark, now to 0x23
    assert_eq!(lut_group1(&pairs[5].1), balanced(5)); // Light, now to 0x22
}
epd_test!(test_sfe213zhbw_gray4_refresh, sfe213zhbw_gray4_refresh_body);

/// `Balanced` runs at 100 Hz and drives only changed pixels: white→black (WK, `0x23`) pushes
/// white then black, black→white (KW, `0x22`) black then white, 20 frames (200 ms) each by
/// default. White→white and black→black get empty tables.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_balanced_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut controller = controller.with_refresh_mode(Uc8253RefreshMode::Balanced);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        commands,
        vec![0x30, 0x20, 0x21, 0x24, 0x22, 0x23, 0x17, 0x30]
    );
    assert_eq!(pairs[0].1, vec![0x13]); // 100 Hz
    assert_eq!(lut_group1(&pairs[1].1), [0x01, 0x14, 0x14, 0x00, 0x00, 0x01, 0x01]);
    assert_eq!(pairs[2].1, vec![0x00; 42]); // WW: untouched
    assert_eq!(pairs[3].1, vec![0x00; 42]); // KK: untouched
    assert_eq!(lut_group1(&pairs[4].1), [0x01, 0x54, 0x94, 0x00, 0x00, 0x01, 0x01]); // KW
    assert_eq!(lut_group1(&pairs[5].1), balanced(20)); // WK
    assert_eq!(pairs[7].1, vec![0x09]); // back to 50 Hz

    assert_eq!(controller.balanced_ms(), 200);
    assert_eq!(controller.with_balanced_ms(155).balanced_ms(), 160);
}
epd_test!(test_sfe213zhbw_balanced_refresh, sfe213zhbw_balanced_refresh_body);

#[test]
fn test_sfe213zhbw_gray_black_ms_rounds_and_clamps() {
    let controller = Uc8253Controller::new(Uc8252c213::WIDTH, Uc8252c213::HEIGHT);
    assert_eq!(controller.gray4_black_ms(), 160);
    assert_eq!(controller.grayscale_black_ms(), 160);
    assert_eq!(controller.with_gray4_black_ms(124).gray4_black_ms(), 120);
    assert_eq!(controller.with_grayscale_black_ms(124).grayscale_black_ms(), 120);
    assert_eq!(controller.with_grayscale_black_ms(125).grayscale_black_ms(), 130);
    assert_eq!(controller.with_grayscale_black_ms(0).grayscale_black_ms(), 10);
    assert_eq!(controller.with_grayscale_black_ms(5000).grayscale_black_ms(), 630);
    assert_eq!(controller.with_gray4_black_ms(u16::MAX).gray4_black_ms(), 630);
}

/// Records the commands `clear_to_white` sends for `clear`, with their payloads.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn clear_to_white_pairs(clear: Uc8253WhiteClear) -> Vec<(u8, Vec<u8>)> {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, mut controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut delay = DummyDelay;
    controller
        .clear_to_white(&mut bus, &mut delay, clear)
        .await
        .unwrap();
    let pairs = command_payloads(&bus_backend.records.borrow());
    pairs
}

/// Each `Uc8253WhiteClear` writes the NEW plane white and keeps the OLD plane, except
/// `BalancedFlash`, which also whites the OLD plane so every pixel gets the even flash.
/// `Mono` refreshes with the DU tables, the flashes with GC, and `AlreadyWhite` sends nothing.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_clear_to_white_body() {
    const LUTS: [u8; 6] = [0x20, 0x21, 0x24, 0x22, 0x23, 0x17];
    let commands = |pairs: &[(u8, Vec<u8>)]| pairs.iter().map(|(c, _)| *c).collect::<Vec<u8>>();

    let mono = clear_to_white_pairs(Uc8253WhiteClear::Mono).await;
    assert_eq!(commands(&mono), [&[0x13][..], &LUTS].concat());
    assert_eq!(mono[0].1, vec![0xFF; 4000]);
    assert_eq!(lut_group1(&mono[4].1), [0x01, 0x8F, 0x01, 0x00, 0x00, 0x01, 0x01]); // DU K2W

    let mono_flash = clear_to_white_pairs(Uc8253WhiteClear::MonoFlash).await;
    assert_eq!(commands(&mono_flash), [&[0x13][..], &LUTS].concat());
    assert_eq!(lut_group1(&mono_flash[2].1), [0x01, 0x4F, 0x8F, 0x0F, 0x01, 0x01, 0x01]); // GC W2W

    let balanced_flash = clear_to_white_pairs(Uc8253WhiteClear::BalancedFlash).await;
    assert_eq!(commands(&balanced_flash), [&[0x10, 0x13][..], &LUTS].concat());
    assert_eq!(balanced_flash[0].1, vec![0xFF; 4000]);
    assert_eq!(balanced_flash[1].1, vec![0xFF; 4000]);
    assert_eq!(lut_group1(&balanced_flash[3].1), [0x01, 0x4F, 0x8F, 0x0F, 0x01, 0x01, 0x01]);

    assert!(clear_to_white_pairs(Uc8253WhiteClear::AlreadyWhite).await.is_empty());
}
epd_test!(test_sfe213zhbw_clear_to_white, sfe213zhbw_clear_to_white_body);

/// `draw_grayscale` clears first as asked (here `Mono`: a DU refresh to white), then runs one
/// pass per three gray levels in use, at 100 Hz. With 4 levels and only black and white pixels,
/// the single pass routes black (level 3) to WK (OLD set, NEW clear) and parks white on the empty
/// KW table (OLD clear, NEW set).
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_draw_grayscale_body() {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, mut controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut delay = DummyDelay;

    controller
        .draw_grayscale(&mut bus, &mut delay, Uc8253WhiteClear::Mono, 4, |x, _y| {
            if x < 8 { 0 } else { 255 }
        })
        .await
        .unwrap();

    let pairs = command_payloads(&bus_backend.records.borrow());
    let commands: Vec<u8> = pairs.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        commands,
        vec![
            0x13, // NEW plane white; OLD keeps the image on the glass
            0x20, 0x21, 0x24, 0x22, 0x23, 0x17, // DU clear, at 50 Hz
            0x30, // 100 Hz for the passes
            0x10, 0x13, // pass planes
            0x20, 0x21, 0x24, 0x23, 0x22, 0x17, // pass LUTs, LUT2/LUT3 swapped on this load
            0x30, // back to 50 Hz
        ]
    );
    assert_eq!(pairs[0].1, vec![0xFF; 4000]);
    assert_eq!(lut_group1(&pairs[4].1), [0x01, 0x8F, 0x01, 0x00, 0x00, 0x01, 0x01]); // DU K2W
    assert_eq!(pairs[7].1, vec![0x13]);
    assert_eq!(pairs[16].1, vec![0x09]);

    let mut old_row = [0x00u8; 16];
    old_row[0] = 0xFF;
    let mut new_row = [0xFFu8; 16];
    new_row[0] = 0x00;
    assert_eq!(pairs[8].1, old_row.repeat(250));
    assert_eq!(pairs[9].1, new_row.repeat(250));

    // 160 ms to black is 16 frames at 100 Hz: 5/11/16 frames each way for levels 1/2/3.
    assert_eq!(lut_group1(&pairs[10].1), [0x01, 0x10, 0x10, 0x00, 0x00, 0x01, 0x01]);
    assert_eq!(lut_group1(&pairs[11].1), balanced(5)); // WW: level 1
    assert_eq!(lut_group1(&pairs[12].1), balanced(11)); // KK: level 2
    // Swapped load: register 0x23 receives the (empty) KW table, 0x22 the WK table.
    assert_eq!(pairs[13].1, vec![0x00; 42]);
    assert_eq!(lut_group1(&pairs[14].1), balanced(16)); // WK: level 3
    assert_eq!(pairs[15].1, vec![0xA5]);
}
epd_test!(test_sfe213zhbw_draw_grayscale, sfe213zhbw_draw_grayscale_body);

/// Passes whose three levels no pixel uses are skipped: a uniform mid-gray at 16 levels on an
/// already-white screen needs exactly one refresh, though all five passes stream their planes.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn sfe213zhbw_draw_grayscale_skips_empty_passes_body() {
    let bus_backend = RecordingSpiBus::new();
    let (mut bus, mut controller) = sfe213zhbw_bus_controller(&bus_backend);
    let mut delay = DummyDelay;

    controller
        .draw_grayscale(&mut bus, &mut delay, Uc8253WhiteClear::AlreadyWhite, 16, |_x, _y| 128)
        .await
        .unwrap();

    let pairs = command_payloads(&bus_backend.records.borrow());
    let refreshes = pairs.iter().filter(|(c, _)| *c == 0x17).count();
    assert_eq!(refreshes, 1);
    let plane_writes = pairs.iter().filter(|(c, _)| *c == 0x13).count();
    assert_eq!(plane_writes, 5);
}
epd_test!(
    test_sfe213zhbw_draw_grayscale_skips_empty_passes,
    sfe213zhbw_draw_grayscale_skips_empty_passes_body
);

/// The grayscale entry points are specific to the `Sfe213zhbw` profile and send nothing
/// elsewhere.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_draw_grayscale_unsupported_body() {
    let bus_backend = RecordingSpiBus::new();
    let mut bus = SpiBusWrapper::new(&bus_backend, TestDc(&bus_backend), DummyPin, FixedPin(true));
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    let result = controller
        .draw_grayscale(&mut bus, &mut delay, Uc8253WhiteClear::Mono, 4, |_x, _y| 0)
        .await;
    assert!(matches!(result, Err(EpdBusError::Unsupported)));
    let result = controller
        .clear_to_white(&mut bus, &mut delay, Uc8253WhiteClear::BalancedFlash)
        .await;
    assert!(matches!(result, Err(EpdBusError::Unsupported)));
    assert!(bus_backend.records.borrow().is_empty());
}
epd_test!(
    test_uc8253_draw_grayscale_unsupported,
    uc8253_draw_grayscale_unsupported_body
);

/// `Gray4Polarity::UC8253_SFE213ZHBW` keeps the panel's 0xFF-white RAM: White leaves both
/// planes set, Light clears only plane A (NEW), Dark only plane B (OLD), Black both.
#[test]
fn test_gray4_polarity_uc8253_sfe213zhbw() {
    let mut plane_a = [0u8; 16];
    let mut plane_b = [0u8; 16];
    let mut pair = GrayBufferPair::new(
        &mut plane_a,
        &mut plane_b,
        122,
        1,
        0,
        Gray4Polarity::UC8253_SFE213ZHBW,
    );
    pair.clear();
    pair.set_pixel(1, 0, Gray4Color::Light);
    pair.set_pixel(2, 0, Gray4Color::Dark);
    pair.set_pixel(3, 0, Gray4Color::Black);
    assert_eq!(pair.plane_a().as_slice()[0], 0b1010_1111);
    assert_eq!(pair.plane_b().as_slice()[0], 0b1100_1111);
    assert_eq!(pair.plane_a().as_slice()[1], 0xFF);
}
