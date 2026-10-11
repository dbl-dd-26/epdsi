#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.
//!
//! `Ist7163Controller` drives `GDEM037F51` (Waveshare "3.7inch e-Paper (G)"). The init bytes are
//! those of Waveshare's `EPD_3in7g.cpp` (V1.0, 2025-02-26) and Good Display's
//! `A32-GDEM037F51-20230926` demo, which agree on every byte; all but `E7`, `E3`, `E9` and the
//! last two `POFS` bytes were read against `IST7163CA1-C-20231220.pdf`. These tests pin the exact
//! byte stream and the delays.
//!
//! BUSY is active-low (busy while LOW), so `FixedPin(true)` backs BUSY, as in the JD79661 tests.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_ist7163_gdem037f51_panel_dimensions() {
    assert_eq!(GDEM037F51::WIDTH, 240);
    assert_eq!(GDEM037F51::HEIGHT, 416);
    assert_eq!(GDEM037F51::COLOR_MODE, ColorMode::QuadColor);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ist7163Controller::new(GDEM037F51::WIDTH, GDEM037F51::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // Good Display `EPD_init()`: 100 ms before reset, then 50 ms low and 50 ms high.
    // `hard_reset` is 5 ms settle, low phase, high phase. The last 100 ms is the settle before
    // polling BUSY after PON (Waveshare `ReadBusyH`).
    assert_eq!(delay.calls_ms, vec![100, 5, 50, 50, 100]);

    let cmd = |c: u8, d: &[u8]| vec![SpiRecord::Command(c), SpiRecord::Data(d.to_vec())];
    let mut expected: Vec<SpiRecord> = Vec::new();
    expected.extend(cmd(0x00, &[0x0F, 0x29])); // PSR
    expected.extend(cmd(0x01, &[0x07, 0x00, 0x22, 0x78, 0x0A, 0x22])); // PWR
    expected.extend(cmd(0x03, &[0x10, 0x54, 0x44])); // POFS
    expected.extend(cmd(0x06, &[0xC0, 0xC0, 0xC0])); // BTST
    expected.extend(cmd(0x30, &[0x08])); // PLL
    expected.extend(cmd(0x41, &[0x00])); // TSE
    expected.extend(cmd(0x50, &[0x37])); // CDI
    expected.extend(cmd(0x60, &[0x02, 0x02])); // TCON
    expected.extend(cmd(0x61, &[0x00, 0xF0, 0x01, 0xA0])); // TRES: 240 x 416
    expected.extend(cmd(0x65, &[0x00, 0x00, 0x00, 0x00])); // GSST
    expected.extend(cmd(0xE7, &[0x1C])); // vendor-only
    expected.extend(cmd(0xE3, &[0x22])); // vendor-only
    expected.extend(cmd(0xFF, &[0xA5])); // enter test mode
    expected.extend(cmd(0xEF, &[0x01, 0x1E, 0x0A, 0x1B, 0x0B, 0x17]));
    expected.extend(cmd(0xC3, &[0xFD]));
    expected.extend(cmd(0xDC, &[0x01]));
    expected.extend(cmd(0xDD, &[0x08]));
    expected.extend(cmd(0xDE, &[0x41]));
    expected.extend(cmd(0xFD, &[0x01]));
    expected.extend(cmd(0xE8, &[0x03]));
    expected.extend(cmd(0xDA, &[0x07]));
    expected.extend(cmd(0xC9, &[0x00]));
    expected.extend(cmd(0xA8, &[0x0F]));
    expected.extend(cmd(0xFF, &[0xE3])); // quit test mode
    expected.extend(cmd(0xE9, &[0x01])); // vendor-only
    expected.push(SpiRecord::Command(0x04)); // POWER_ON, bare
    expected.extend(cmd(0xFF, &[0xA5])); // second test bank
    expected.extend(cmd(0xEF, &[0x03, 0x1E, 0x0A, 0x1B, 0x0E, 0x15]));
    expected.extend(cmd(0xDC, &[0x01]));
    expected.extend(cmd(0xDD, &[0x08]));
    expected.extend(cmd(0xDE, &[0x41]));
    expected.extend(cmd(0xFF, &[0xE3]));

    assert_eq!(bus_backend.records.borrow().clone(), expected);
}
epd_test!(test_ist7163_init_sequence, ist7163_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_tres_rounds_width_up_to_a_multiple_of_four_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    // HRES low two bits are fixed at zero (spec R61H), so 242 must go out as 244 = 0x00F4.
    let mut controller = Ist7163Controller::new(242, 300);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    let at = records
        .iter()
        .position(|r| *r == SpiRecord::Command(0x61))
        .unwrap();
    assert_eq!(
        records[at + 1],
        SpiRecord::Data(vec![0x00, 0xF4, 0x01, 0x2C])
    );
}
epd_test!(
    test_ist7163_tres_rounds_width_up_to_a_multiple_of_four,
    ist7163_tres_rounds_width_up_to_a_multiple_of_four_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_write_frame_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ist7163Controller::new(GDEM037F51::WIDTH, GDEM037F51::HEIGHT);

    controller
        .write_frame(
            &mut bus,
            ColorChannel::BlackWhite,
            &[0x00, 0x55, 0xAA, 0xFF],
        )
        .await
        .unwrap();

    // One plane on 0x10, as the demo's `EPD_3IN7G_Display`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x00, 0x55, 0xAA, 0xFF])
        ]
    );
}
epd_test!(test_ist7163_write_frame, ist7163_write_frame_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_trigger_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ist7163Controller::new(GDEM037F51::WIDTH, GDEM037F51::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    // R12H takes one parameter, `00h`. No POWER_OFF here: both demos refresh and then sleep.
    assert_eq!(delay.calls_ms, vec![100]);
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
}
epd_test!(test_ist7163_trigger_refresh, ist7163_trigger_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ist7163Controller::new(GDEM037F51::WIDTH, GDEM037F51::HEIGHT);
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    // Good Display `EPD_sleep()` waits 100 ms between POWER_OFF and DEEP_SLEEP, marked
    // "necessary". Waveshare's `EPD_3IN7G_Sleep()` has no such delay; the longer one is kept.
    assert_eq!(delay.calls_ms, vec![100]);
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
epd_test!(test_ist7163_sleep, ist7163_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ist7163_clear_frame_uses_the_correct_2bpp_byte_count_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ist7163Controller::new(GDEM037F51::WIDTH, GDEM037F51::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEM037F51>::new(controller).build(bus);
    let mut delay = DummyDelay;

    driver.init(&mut delay).await.unwrap();
    bus_backend.records.borrow_mut().clear();

    // 240 px * 2 bits / 8 * 416 rows = 24,960 bytes, the demo's `Width * Height` in
    // `EPD_3IN7G_Clear` with `Width = 240 / 4`.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0x55)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x55; 24_960])
        ]
    );
}
epd_test!(
    test_ist7163_clear_frame_uses_the_correct_2bpp_byte_count,
    ist7163_clear_frame_uses_the_correct_2bpp_byte_count_body
);
