//! Panel specification for GDEY0266T90H (2.66" 184 x 360 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Good Display `GDEY0266T90H`, FPC marking `FPC-H011`. A different glass
//!   from [`GDEY0266T90`](crate::panels::GDEY0266T90): same 2.66" size, but a higher resolution
//!   raster behind an SSD1685 instead of an SSD1680. The two are not interchangeable.
//! - **Controller IC**: SSD1685, driven through
//!   [`Ssd168xVariant::Ssd1685`](crate::controllers::Ssd168xVariant::Ssd1685). The variant is
//!   **not** inferred from the panel type (`EpdController` never sees `PANEL`), so build the
//!   controller with `Ssd168xController::for_panel::<GDEY0266T90H>(Ssd168xVariant::Ssd1685)`.
//!   Pairing the panel with the SSD1680 or SSD1681 variant does not error. It writes `0x21`
//!   (SSD1680) or refreshes with the wrong bytes and renders badly.
//! - **Native Resolution**: 184 x 360 pixels (spec: 184(H) x 360(V), 30.69 x 60.05 mm active
//!   area). The SSD1685 supports 168/184/200/216 source outputs; the panel's OTP selects 184.
//!   `0x21` byte 2 bits 7-6 would select it explicitly (`01` = 184, POR `00` = 200), but Good
//!   Display's demo never writes `0x21`, and neither does this variant.
//! - **RAM Alignment**: none needed. 184 is a multiple of 8, so a row is 23 bytes and a frame
//!   8280 bytes. X window end byte is `0x16`.
//! - **Busy Polarity**: Active-**HIGH** (busy while HIGH), per the spec's pin table.
//! - **Reference sources**: Good Display `A32-GDEY0266T90H-2FP-20230915` demo (init, `0xF4`
//!   full, `0xC7` fast, `0x1C` partial, deep sleep with a 100 ms settle) and the SSD1685 datasheet
//!   Rev 0.11. GxEPD2 has no `GDEY0266T90H` class, but its `GxEPD2_290_GDEY029T71H` drives the
//!   same IC and supplies the `0xDC` partial byte.
//!
//! ### Vendor-only bytes
//! Not in the SSD1685 Rev 0.11 command table, kept for parity with the vendor demo:
//! - `0x1A` (temperature register) is documented with **one** parameter byte; the vendor demo and
//!   this crate send two (`6E 00` in `FastFull`).
//! - `0x3F` is documented as "set this byte to 22h". Neither the vendor demo nor this variant
//!   writes it.
//!
//! ### Status
//! Register sequence derived from the vendor demo and datasheet above. **Confirmed on physical
//! hardware** (11 Oct 2026): the `GDEY0266T90H` examples ran and rendered correctly through `epdsi`
//! on RP2350 (blocking and async), RP2040 and ESP32-C3. They exercise `Full` and `Partial`, so the
//! `0xF7` full and `0xDC` partial bytes are bench-confirmed. Measured on the Feather RP2040
//! ThinkInk: `Full` 1191 ms and `Partial` 356 ms per update. The Pico 2 (RP2350) gives the same
//! 1191 ms and 356 ms blocking, and 1190 ms and 355 ms async. The XIAO ESP32-C3 is within 2 ms
//! (1193 ms and 356-357 ms). Across all four runs the spread is under 3 ms, so the time is set by
//! the panel, not the host or the blocking/async API. Both are far
//! faster than the SSD1680 [`GDEY0266T90`](crate::panels::GDEY0266T90), which took about 4.1 s for
//! both. `FastFull` (the vendor's `6E 00`
//! temperature override) measured 1678 ms against 1191 ms for `Full` on the Pico 2, the Feather
//! RP2040 and the XIAO ESP32-C3 (1677 ms against 1190 ms async), so it is slower on this panel at
//! room temperature, with no visible difference in image quality on any board. There is no reason
//! to use it here. Stay on `Full`.
//!
//! ### Vendor References
//! - Good Display product page: <https://www.good-display.com/product/501.html>
//! - Specification: <https://v4.cecdn.yun300.cn/100001_1909185148/GDEY0266T90H.pdf>
//! - SSD1685 datasheet: <https://v4.cecdn.yun300.cn/100001_1909185148/SSD1685.pdf>
//! - Demo: <https://v4.cecdn.yun300.cn/100001_1909185148/A32-GDEY0266T90H-2FP-20230915.rar>
//! - GxEPD2 same-IC reference: <https://github.com/ZinggJM/GxEPD2/blob/master/src/gdey/GxEPD2_290_GDEY029T71H.cpp>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for the Good Display GDEY0266T90H (SSD1685, 184 x 360).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY0266T90H;

impl EpdPanel for GDEY0266T90H {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 184;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 360;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}
