#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async), see `tests/support/mod.rs`.
//!
//! Every expected byte stream below is Good Display's own `AU-GDEW0215T12-P-20230926` demo,
//! except where a test says the datasheet was followed instead.

use epdsi::prelude::*;

mod support;
use support::*;

/// The vendor demo's partial LUT: `[first, 0x19, 0x01, 0x00, 0x00, 0x01]` then zeros.
fn lut(first: u8, len: usize) -> Vec<u8> {
    let mut table = vec![0u8; len];
    table[0] = first;
    table[1] = 0x19;
    table[2] = 0x01;
    table[5] = 0x01;
    table
}

#[test]
fn test_uc8151d_gdew0215t12_panel_dimensions() {
    assert_eq!(GDEW0215T12::WIDTH, 112);
    assert_eq!(GDEW0215T12::HEIGHT, 208);
    // 112 / 8 = 14 bytes per row, 208 rows: the vendor's EPD_ARRAY.
    assert_eq!(GDEW0215T12::WIDTH.div_ceil(8) * GDEW0215T12::HEIGHT, 2912);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_full_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // `EPD_Init`: PSR `1F 0D`, TRES `70 00 D0`, PON, CDI `97`. No LUT upload, no BTST, no PWR.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
            SpiRecord::Command(0x61),
            SpiRecord::Data(vec![0x70, 0x00, 0xD0]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
        ]
    );
    // Three reset pulses, each 5 ms high, then 10 ms low and 10 ms high.
    assert_eq!(delay.calls_ms, vec![5, 10, 10, 5, 10, 10, 5, 10, 10]);
}
epd_test!(test_uc8151d_full_init_sequence, uc8151d_full_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_partial_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT)
        .with_refresh_mode(Uc8151dRefreshMode::Partial);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // `EPD_Init_Part`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x01),
            SpiRecord::Data(vec![0x03, 0x00, 0x2B, 0x2B, 0x03]),
            SpiRecord::Command(0x06),
            SpiRecord::Data(vec![0x17, 0x17, 0x17]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0xBF, 0x0D]),
            SpiRecord::Command(0x30),
            SpiRecord::Data(vec![0x3C]),
            SpiRecord::Command(0x61),
            SpiRecord::Data(vec![0x70, 0x00, 0xD0]),
            SpiRecord::Command(0x82),
            SpiRecord::Data(vec![0x12]),
            SpiRecord::Command(0x20),
            SpiRecord::Data(lut(0x00, 44)),
            SpiRecord::Command(0x21),
            SpiRecord::Data(lut(0x00, 42)),
            SpiRecord::Command(0x22),
            SpiRecord::Data(lut(0x80, 42)),
            SpiRecord::Command(0x23),
            SpiRecord::Data(lut(0x40, 42)),
            SpiRecord::Command(0x24),
            SpiRecord::Data(lut(0x00, 42)),
            SpiRecord::Command(0x04),
        ]
    );
}
epd_test!(
    test_uc8151d_partial_init_sequence,
    uc8151d_partial_init_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_full_frame_write_and_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut delay = RecordingDelay::new();

    // `EPD_WhiteScreen_ALL`: old plane `0x10` first, then new plane `0x13`, then `0x12`.
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xFF, 0xFF])
        .await
        .unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xA5, 0x5A])
        .await
        .unwrap();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xFF, 0xFF]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xA5, 0x5A]),
            SpiRecord::Command(0x12),
        ]
    );
    // The demo's necessary 1 ms gap, then a wait for BUSY to assert.
    assert_eq!(delay.calls_ms.first(), Some(&1));
    // Blocking polls in 1 ms steps, and the test pin never asserts, so the whole 500 ms budget
    // runs. Async waits on the pin edge and spends no delay.
    #[cfg(feature = "blocking")]
    assert_eq!(delay.calls_ms.len(), 1 + 500);
    #[cfg(not(feature = "blocking"))]
    assert_eq!(delay.calls_ms, vec![1]);
}
epd_test!(
    test_uc8151d_full_frame_write_and_refresh,
    uc8151d_full_frame_write_and_refresh_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_partial_window_session_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut delay = DummyDelay;

    // x 10..=30 rounds out to banks 8..=31, y 40..=71 is sent inclusive.
    controller
        .set_window(&mut bus, 10, 40, 30, 71)
        .await
        .unwrap();
    assert!(
        bus_backend.records.borrow().is_empty(),
        "set_window must not touch the bus"
    );

    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xFF])
        .await
        .unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0x00])
        .await
        .unwrap();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    // One partial-in session spans both plane writes and the refresh, like the vendor's
    // `EPD_Dis_Part`, then the chip leaves partial mode after the refresh.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91),
            SpiRecord::Command(0x90),
            SpiRecord::Data(vec![0x08, 0x1F, 0x00, 40, 0x00, 71, 0x28]),
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xFF]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x92),
        ]
    );
}
epd_test!(
    test_uc8151d_partial_window_session,
    uc8151d_partial_window_session_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_full_panel_window_is_not_partial_mode_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut delay = DummyDelay;

    // A window spanning the whole panel is full-frame addressing: the vendor's full update never
    // sends `PARTIAL_IN`, so neither may this.
    controller
        .set_window(
            &mut bus,
            0,
            0,
            GDEW0215T12::WIDTH - 1,
            GDEW0215T12::HEIGHT - 1,
        )
        .await
        .unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0x5A])
        .await
        .unwrap();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x12),
        ]
    );
}
epd_test!(
    test_uc8151d_full_panel_window_is_not_partial_mode,
    uc8151d_full_panel_window_is_not_partial_mode_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_window_change_and_clear_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);

    controller.set_window(&mut bus, 0, 0, 15, 7).await.unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0x11])
        .await
        .unwrap();

    // A different window while already in partial mode re-sends only the window.
    controller.set_window(&mut bus, 16, 8, 31, 15).await.unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0x22])
        .await
        .unwrap();

    // Clearing the window leaves partial mode before the next write.
    controller.clear_window();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0x33])
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91),
            SpiRecord::Command(0x90),
            SpiRecord::Data(vec![0x00, 0x0F, 0x00, 0, 0x00, 7, 0x28]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0x11]),
            SpiRecord::Command(0x90),
            SpiRecord::Data(vec![0x10, 0x1F, 0x00, 8, 0x00, 15, 0x28]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0x22]),
            SpiRecord::Command(0x92),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0x33]),
        ]
    );
}
epd_test!(
    test_uc8151d_window_change_and_clear,
    uc8151d_window_change_and_clear_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_pattern_write_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);

    controller
        .write_frame_pattern(&mut bus, ColorChannel::RedYellow, 0xFF, 2912)
        .await
        .unwrap();

    let records = coalesce(&bus_backend.records.borrow());
    assert_eq!(
        records,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xFF; 2912])]
    );
}
epd_test!(test_uc8151d_pattern_write, uc8151d_pattern_write_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    // `EPD_DeepSleep`: CDI `F7`, POF, BUSY wait, a 100 ms gap the demo calls necessary, DSLP `A5`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
    assert_eq!(delay.calls_ms, vec![100]);
}
epd_test!(test_uc8151d_sleep, uc8151d_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8151d_driver_smoke_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    let mut epd = EpdBuilder::<_, GDEW0215T12>::new(controller).build(bus);
    let mut delay = DummyDelay;

    epd.init(&mut delay).await.expect("init failed");
    epd.set_window(0, 0, GDEW0215T12::WIDTH - 1, GDEW0215T12::HEIGHT - 1)
        .await
        .expect("set_window failed");
    epd.refresh(&mut delay).await.expect("refresh failed");
    epd.sleep(&mut delay).await.expect("sleep failed");
}
epd_test!(test_uc8151d_driver_smoke, uc8151d_driver_smoke_body);

#[test]
fn test_uc8151d_refresh_mode_defaults_to_full() {
    let controller = Uc8151dController::new(GDEW0215T12::WIDTH, GDEW0215T12::HEIGHT);
    assert_eq!(controller.refresh_mode(), Uc8151dRefreshMode::Full);
}
