#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async), see `tests/support/mod.rs`.
//!
//! `Ssd2681Controller` drives `GDEM0154F61H`. The byte streams below replay Good Display's
//! `AU-GDEM0154F61H-F-20260513` demo (`EPD_init`, `EPD_init_Fast`, `EPD_update`, `EPD_sleep`).
//! BUSY is active-low (busy while LOW), so `FixedPin(true)` backs BUSY.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_ssd2681_gdem0154f61h_panel_dimensions() {
    assert_eq!(GDEM0154F61H::WIDTH, 200);
    assert_eq!(GDEM0154F61H::HEIGHT, 200);
    assert_eq!(GDEM0154F61H::COLOR_MODE, ColorMode::QuadColor);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd2681_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ssd2681Controller::new();
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // Demo `EPD_init()`: 100 ms, then reset low 50 ms and high 50 ms (5 ms settle first).
    assert_eq!(delay.calls_ms, vec![100, 5, 50, 50]);
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x04),
        ]
    );
}
epd_test!(test_ssd2681_init_sequence, ssd2681_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd2681_fast_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ssd2681Controller::new().with_refresh_mode(Ssd2681RefreshMode::Fast);
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // Demo `EPD_init_Fast()`: 20 ms, then reset low 50 ms and high 50 ms.
    assert_eq!(delay.calls_ms, vec![20, 5, 50, 50]);
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0xEF),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0xF6),
            SpiRecord::Data(vec![0x24]),
            SpiRecord::Command(0xEF),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE6),
            SpiRecord::Data(vec![0x5C]),
            SpiRecord::Command(0xA5),
            SpiRecord::Command(0x04),
        ]
    );
}
epd_test!(
    test_ssd2681_fast_init_sequence,
    ssd2681_fast_init_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd2681_frame_refresh_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ssd2681Controller::new();
    let mut delay = RecordingDelay::new();

    controller
        .write_frame(
            &mut bus,
            ColorChannel::BlackWhite,
            &[0x00, 0x55, 0xAA, 0xFF],
        )
        .await
        .unwrap();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::RedYellow, 0x55, 3)
        .await
        .unwrap();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    controller.sleep(&mut bus, &mut delay).await.unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x00, 0x55, 0xAA, 0xFF]),
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x55, 0x55, 0x55]),
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
    test_ssd2681_frame_refresh_sleep,
    ssd2681_frame_refresh_sleep_body
);
