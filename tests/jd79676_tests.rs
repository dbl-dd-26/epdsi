#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.
//!
//! `Jd79676Controller` wraps `Jd7966xController`'s `Jd7966xVariant::Jd79676` arm, which drives
//! `GDEY0213F52`. Good Display's `A32-GDEY0213F52-20240827` demo (V2.0) programs only `0xE9 0x01`
//! before `PON`, so the IC runs from OTP settings. These tests pin that exact byte stream.
//!
//! `Jd7966xRefreshMode::Fast` adds Good Display's `EPD_init_Fast()` bytes (`E0 02`, `E6 5A`, bare
//! `A5`) and its shorter reset timing. The refresh and sleep paths do not change.
//!
//! BUSY is active-low (busy while LOW), so `FixedPin(true)` backs BUSY, as in the JD79661 tests.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_jd79676_gdey0213f52_panel_dimensions() {
    assert_eq!(GDEY0213F52::WIDTH, 122);
    assert_eq!(GDEY0213F52::HEIGHT, 250);
    assert_eq!(GDEY0213F52::RAM_WIDTH, 128);
    assert_eq!(GDEY0213F52::COLOR_MODE, ColorMode::QuadColor);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // Demo `EPD_init()`: 20 ms wait, then reset with at least 40 ms low and 50 ms high.
    // `hard_reset` is 5 ms settle, then the low phase, then the high phase.
    assert_eq!(delay.calls_ms, vec![20, 5, 50, 50]);

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE9), // undocumented in the public datasheet, in the demo
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x04), // POWER_ON, bare. No PSR/PWR/BTST/CDI/TCON/TRES.
        ]
    );
}
epd_test!(test_jd79676_init_sequence, jd79676_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_write_frame_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);

    controller
        .write_frame(
            &mut bus,
            ColorChannel::BlackWhite,
            &[0x00, 0x55, 0xAA, 0xFF],
        )
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x00, 0x55, 0xAA, 0xFF])
        ]
    );
}
epd_test!(test_jd79676_write_frame, jd79676_write_frame_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_trigger_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    // R12H takes one data byte, `00h` (JD79676AA v1.0.4 section 8.2.9, demo `EPD_update()`).
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
}
epd_test!(test_jd79676_trigger_refresh, jd79676_trigger_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    // Unlike the `GDEY0213F51` demo, the F52 demo's `EPD_sleep()` has no 100 ms delay between
    // POWER_OFF and DEEP_SLEEP.
    assert!(delay.calls_ms.is_empty());
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}
epd_test!(test_jd79676_sleep, jd79676_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_clear_frame_uses_the_correct_2bpp_byte_count_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY0213F52>::new(controller).build(bus);
    let mut delay = DummyDelay;

    driver.init(&mut delay).await.unwrap();
    bus_backend.records.borrow_mut().clear();

    // 128 px RAM width * 2 bits / 8 * 250 rows = 8000 bytes, the demo's `EPD_ARRAY`.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0x55)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x55; 8_000])]
    );
}
epd_test!(
    test_jd79676_clear_frame_uses_the_correct_2bpp_byte_count,
    jd79676_clear_frame_uses_the_correct_2bpp_byte_count_body
);

#[test]
fn test_jd79676_refresh_mode_defaults_to_full() {
    let controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT);
    assert_eq!(controller.refresh_mode(), Jd7966xRefreshMode::Full);

    let controller = controller.with_refresh_mode(Jd7966xRefreshMode::Fast);
    assert_eq!(controller.refresh_mode(), Jd7966xRefreshMode::Fast);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_fast_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT)
        .with_refresh_mode(Jd7966xRefreshMode::Fast);
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // Demo `EPD_init_Fast()`: 100 ms wait, then reset with 10 ms low and 10 ms high.
    // `hard_reset` is 5 ms settle, then the low phase, then the high phase.
    assert_eq!(delay.calls_ms, vec![100, 5, 10, 10]);

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE0), // CCSET, `TSFIX`
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE6), // undocumented in the public datasheet, in the demo
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0xA5), // bare, no data byte
            SpiRecord::Command(0xE9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x04), // POWER_ON
        ]
    );
}
epd_test!(
    test_jd79676_fast_init_sequence,
    jd79676_fast_init_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79676_fast_mode_leaves_refresh_and_sleep_unchanged_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79676Controller::new(GDEY0213F52::WIDTH, GDEY0213F52::HEIGHT)
        .with_refresh_mode(Jd7966xRefreshMode::Fast);
    let mut delay = RecordingDelay::new();

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    controller.sleep(&mut bus, &mut delay).await.unwrap();

    assert!(delay.calls_ms.is_empty());
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x12),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}
epd_test!(
    test_jd79676_fast_mode_leaves_refresh_and_sleep_unchanged,
    jd79676_fast_mode_leaves_refresh_and_sleep_unchanged_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd7966x_fast_mode_is_ignored_off_jd79676_body() {
    // The mode only acts on the Jd79676 variant. JD79660 and JD79661 must produce the same
    // bytes and delays under `Fast` as under `Full`.
    for variant in [Jd7966xVariant::Jd79660, Jd7966xVariant::Jd79661] {
        let mut streams = Vec::new();
        for mode in [Jd7966xRefreshMode::Full, Jd7966xRefreshMode::Fast] {
            let bus_backend = RecordingSpiBus::new();
            let dc = TestDc(&bus_backend);
            let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
            let mut controller = Jd7966xController::new(200, 200, variant).with_refresh_mode(mode);
            let mut delay = RecordingDelay::new();
            controller
                .init_sequence(&mut bus, &mut delay)
                .await
                .unwrap();
            streams.push((bus_backend.records.borrow().clone(), delay.calls_ms.clone()));
        }
        assert_eq!(streams[0], streams[1], "{variant:?}");
    }
}
epd_test!(
    test_jd7966x_fast_mode_is_ignored_off_jd79676,
    jd7966x_fast_mode_is_ignored_off_jd79676_body
);
