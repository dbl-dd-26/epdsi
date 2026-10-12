#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

//! Byte-pinned tests for the SSD1685 variant of the SSD168x controller, paired with the
//! `GDEY0266T90H` panel. Each expected stream is derived from Good Display's
//! `A32-GDEY0266T90H-2FP-20230915` demo and the SSD1685 datasheet (Rev 0.11), with the two
//! deliberate departures (`0xF7` full, `0xDC` partial) called out where they appear.

use epdsi::controllers::ssd168x::{Ssd168xController, Ssd168xRefreshMode, Ssd168xVariant};
use epdsi::panels::GDEY0266T90H;
use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_ssd1685_gdey0266t90h_panel_dimensions() {
    assert_eq!(GDEY0266T90H::WIDTH, 184);
    assert_eq!(GDEY0266T90H::HEIGHT, 360);
    assert_eq!(GDEY0266T90H::COLOR_MODE, ColorMode::BlackWhite);
}

#[test]
fn test_ssd1685_constructors_select_the_variant() {
    let a = Ssd168xController::new_ssd1685(184, 360);
    let b = Ssd168xController::for_panel::<GDEY0266T90H>(Ssd168xVariant::Ssd1685);
    assert_eq!(a.variant(), Ssd168xVariant::Ssd1685);
    assert_eq!(b.variant(), Ssd168xVariant::Ssd1685);
}

fn controller() -> Ssd168xController {
    Ssd168xController::for_panel::<GDEY0266T90H>(Ssd168xVariant::Ssd1685)
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn record_refresh(mode: Ssd168xRefreshMode) -> Vec<SpiRecord> {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller().with_refresh_mode(mode);
    let mut delay = DummyDelay;
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();
    records
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller();
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x01), // DRIVER_CONTROL: gate lines 360, so 359 = 0x0167, scan 0x00
            SpiRecord::Data(vec![0x67, 0x01, 0x00]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x05]),
            // No 0x21: the vendor demo leaves the OTP's 184-source selection alone.
            SpiRecord::Command(0x18), // TEMP_CONTROL: internal sensor
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE: X increment, Y increment
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44), // SET_RAMXPOS: 184 px is 23 bytes, so 0..=22
            SpiRecord::Data(vec![0x00, 0x16]),
            SpiRecord::Command(0x45), // SET_RAMYPOS: 0..=359
            SpiRecord::Data(vec![0x00, 0x00, 0x67, 0x01]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}
epd_test!(test_ssd1685_init_sequence, ssd1685_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_full_refresh_body() {
    // Full: 0xF7 is the datasheet's "load temperature, display mode 1, disable analog/OSC"
    // sequence. The vendor demo sends 0xF4 (the same without the disable step); that byte is
    // reachable as `Ssd168xRefreshMode::BaseMap`.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::Full).await,
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(test_ssd1685_full_refresh, ssd1685_full_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_base_map_is_the_vendor_full_update_body() {
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::BaseMap).await,
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF4]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1685_base_map_is_the_vendor_full_update,
    ssd1685_base_map_is_the_vendor_full_update_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_partial_refresh_body() {
    // 0xDC, with no hard reset, no border-waveform rewrite and no 0xE0/0x83 power envelope.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::Partial).await,
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xDC]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(test_ssd1685_partial_refresh, ssd1685_partial_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_fast_full_refresh_body() {
    // Vendor `EPD_HW_Init_Fast()` then `EPD_Update_Fast()`: load temperature, override the
    // register with 0x6E (the SSD1680 demos use 0x5A), reload the LUT, display on 0xC7.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::FastFull).await,
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xB1]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x6E, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x91]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xC7]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1685_fast_full_refresh,
    ssd1685_fast_full_refresh_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1685_sleep_has_100ms_settle_delay_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller();
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
    assert_eq!(delay.calls_ms, vec![100]);
}
epd_test!(
    test_ssd1685_sleep_has_100ms_settle_delay,
    ssd1685_sleep_has_100ms_settle_delay_body
);
