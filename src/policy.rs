//! Ghosting debt: when a partial-refresh loop owes the panel a full refresh.
//!
//! Every partial refresh leaves a little charge behind, and after enough of them the panel shows a
//! faint shadow of earlier images. A full refresh clears it, at the cost of a visible flash and a
//! slower update. [`RefreshPolicy`] counts partial refreshes and tells the caller when the next
//! one should be a full refresh.
//!
//! The policy is a plain counter that the caller owns. It does not touch the bus, the driver or
//! the controller, because each controller has its own refresh-mode enum and the driver has no
//! notion of a mode. The caller asks [`RefreshPolicy::next_refresh`] what kind of refresh is due, maps the
//! answer to its controller's mode, and then writes the frame and refreshes as usual.
//!
//! # Where the cadence comes from
//!
//! The default is [`RefreshPolicy::DEFAULT_PARTIALS_BETWEEN_FULL`], 5 partial refreshes between
//! full ones. Good Display's demos for the `GDEQ0426T82`, `GDEY0426T82`, `GDEW0215T12`,
//! `GDEY0266T90`, `GDEY0266T90H`, `GDEY0266Z90` and `GDEY037T03` all carry the comment "After 5
//! partial refreshes, implement a full screen refresh to clear the ghosting", and Waveshare's
//! 3.7 inch (G) manual says to clear the screen after partially refreshing 5 times. That is a rule
//! of thumb from vendor demos and a support FAQ, not a controller limit, so tune it for your panel
//! and content.
//!
//! # The 24 hour rule is yours
//!
//! Vendors also say to refresh the panel at least every 24 hours, even if the image does not
//! change. `epdsi` has no clock, so the policy counts refreshes only. Time-based refreshes are up
//! to the caller.
//!
//! # Mapping the answer to a controller
//!
//! After a partial refresh, going back to a full one needs only `set_refresh_mode` on the
//! controllers below, with one exception. The first two rows were checked on hardware by drawing
//! the same picture after a partial refresh with and without a fresh `init` and comparing the
//! panel. The others come from reading each controller's `trigger_refresh` and `init_sequence`.
//!
//! | Controller | Partial | Full | Call `init` again before the full refresh? |
//! | :--- | :--- | :--- | :--- |
//! | `Ssd168xController` (SSD1680 monochrome panels) | `Ssd168xRefreshMode::Partial` | `Ssd168xRefreshMode::Full` | No |
//! | `Ssd1677Controller` | `Ssd1677RefreshMode::Partial` | `Ssd1677RefreshMode::Full` | No |
//! | `Uc8253Controller` (`Gdey037t03`) | `Uc8253RefreshMode::Partial` or `FastPartial` | `Uc8253RefreshMode::Full` or `FastFull` | No |
//! | `Uc8151dController` | `Uc8151dRefreshMode::Partial` | `Uc8151dRefreshMode::Full` | **Yes**, `init_sequence` reads the mode |
//!
//! Do not use the policy with panels that have no differential partial refresh:
//!
//! - Tri-Color, Spectra 6 and ACeP panels. `Uc8253Variant::Se0352n14` ignores the refresh mode
//!   altogether.
//! - The `Fast` modes of `Jd7966xRefreshMode` and `Ssd2681RefreshMode`. They run a full waveform
//!   at a forced temperature and leave no partial-refresh debt.
//!
//! `PervasiveRefreshMode::Fast` is a differential update, but its vendor cadence and its `init`
//! behaviour have not been checked, so treat it as unverified.
//!
//! # Example
//!
//! ```
//! use epdsi::policy::{RefreshKind, RefreshPolicy};
//! use epdsi::prelude::*;
//!
//! let mut policy = RefreshPolicy::default();
//! let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
//!
//! for _ in 0..7 {
//!     let mode = match policy.next_refresh() {
//!         RefreshKind::Partial => Ssd1677RefreshMode::Partial,
//!         _ => Ssd1677RefreshMode::Full,
//!     };
//!     controller.set_refresh_mode(mode);
//!     // Write the frame with `EpdDriver::write_frame`, then call `EpdDriver::refresh`.
//! }
//! ```

/// The kind of refresh a [`RefreshPolicy`] says is due.
///
/// This enum is `#[non_exhaustive]`, so a `match` outside this crate needs a wildcard arm. That
/// leaves room for another kind later without breaking callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RefreshKind {
    /// A full refresh on the panel's full waveform. Clears accumulated ghosting.
    Full,
    /// A partial refresh. Fast and flash-free, and it adds one to the ghosting count.
    Partial,
}

/// Counts partial refreshes and says when a full refresh is due. See the [module docs](self) for
/// the cadence, the per-controller mapping and the cases where not to use it.
///
/// A new policy assumes the panel was just fully refreshed, so the first [`next_refresh`](Self::next_refresh) call
/// returns [`RefreshKind::Partial`] unless the cadence is 0. If you issue a full refresh yourself,
/// for example the first image after `init` or a manual clear, call [`reset`](Self::reset) so the
/// count starts over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshPolicy {
    partials_between_full: u32,
    partials_since_full: u32,
}

impl RefreshPolicy {
    /// Partial refreshes between full ones when you use [`RefreshPolicy::default`]. See the
    /// [module docs](self) for where 5 comes from.
    pub const DEFAULT_PARTIALS_BETWEEN_FULL: u32 = 5;

    /// A policy that asks for a full refresh after `partials_between_full` partial refreshes.
    ///
    /// With 5, the sequence is five partial refreshes, one full, then five partial again. With 0,
    /// every refresh is a full one. Some libraries use 0 to switch the limit off, so to never ask
    /// for a full refresh, pass `u32::MAX` or do not call the policy.
    pub const fn new(partials_between_full: u32) -> Self {
        RefreshPolicy {
            partials_between_full,
            partials_since_full: 0,
        }
    }

    /// Returns the kind of refresh that is due now and updates the count.
    ///
    /// A partial answer adds one to the count. A full answer resets it to zero, on the
    /// assumption that the caller performs that full refresh.
    #[must_use = "the answer says which refresh mode to use"]
    pub fn next_refresh(&mut self) -> RefreshKind {
        if self.partials_since_full >= self.partials_between_full {
            self.partials_since_full = 0;
            RefreshKind::Full
        } else {
            self.partials_since_full = self.partials_since_full.saturating_add(1);
            RefreshKind::Partial
        }
    }

    /// Partial refreshes counted since the last full one.
    pub const fn partials_since_full(&self) -> u32 {
        self.partials_since_full
    }

    /// The cadence this policy was built with.
    pub const fn partials_between_full(&self) -> u32 {
        self.partials_between_full
    }

    /// Forgets the count, for a full refresh the caller issued without asking
    /// [`next_refresh`](Self::next_refresh).
    pub fn reset(&mut self) {
        self.partials_since_full = 0;
    }
}

impl Default for RefreshPolicy {
    /// A policy with [`RefreshPolicy::DEFAULT_PARTIALS_BETWEEN_FULL`].
    fn default() -> Self {
        RefreshPolicy::new(Self::DEFAULT_PARTIALS_BETWEEN_FULL)
    }
}
