#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `RefreshPolicy` is a plain counter with no `.await`, so these run identically under
//! `cargo test` and `cargo test --no-default-features --features graphics`.

use epdsi::prelude::*;

fn run(policy: &mut RefreshPolicy, calls: usize) -> Vec<RefreshKind> {
    (0..calls).map(|_| policy.next_refresh()).collect()
}

#[test]
fn default_cadence_is_five_partials_then_a_full() {
    let mut policy = RefreshPolicy::default();
    assert_eq!(policy.partials_between_full(), 5);
    assert_eq!(RefreshPolicy::DEFAULT_PARTIALS_BETWEEN_FULL, 5);

    use RefreshKind::{Full, Partial};
    assert_eq!(
        run(&mut policy, 13),
        vec![
            Partial, Partial, Partial, Partial, Partial, Full, Partial, Partial, Partial, Partial,
            Partial, Full, Partial,
        ]
    );
}

#[test]
fn a_new_policy_assumes_the_panel_was_just_refreshed() {
    let mut policy = RefreshPolicy::new(3);
    assert_eq!(policy.partials_since_full(), 0);
    assert_eq!(policy.next_refresh(), RefreshKind::Partial);
    assert_eq!(policy.partials_since_full(), 1);
}

#[test]
fn full_answer_resets_the_count() {
    let mut policy = RefreshPolicy::new(2);
    assert_eq!(policy.next_refresh(), RefreshKind::Partial);
    assert_eq!(policy.next_refresh(), RefreshKind::Partial);
    assert_eq!(policy.partials_since_full(), 2);
    assert_eq!(policy.next_refresh(), RefreshKind::Full);
    assert_eq!(policy.partials_since_full(), 0);
}

#[test]
fn zero_cadence_makes_every_refresh_full() {
    let mut policy = RefreshPolicy::new(0);
    assert_eq!(run(&mut policy, 4), vec![RefreshKind::Full; 4]);
    assert_eq!(policy.partials_since_full(), 0);
}

#[test]
fn reset_starts_the_count_over() {
    let mut policy = RefreshPolicy::new(3);
    run(&mut policy, 3);
    assert_eq!(policy.partials_since_full(), 3);

    policy.reset();
    assert_eq!(policy.partials_since_full(), 0);
    assert_eq!(policy.partials_between_full(), 3, "reset keeps the cadence");

    use RefreshKind::{Full, Partial};
    assert_eq!(
        run(&mut policy, 5),
        vec![Partial, Partial, Partial, Full, Partial]
    );
}

#[test]
fn huge_cadence_never_wraps() {
    let mut policy = RefreshPolicy::new(u32::MAX);
    for _ in 0..10 {
        assert_eq!(policy.next_refresh(), RefreshKind::Partial);
    }
    assert_eq!(policy.partials_since_full(), 10);
}

#[test]
fn policies_are_independent_copies() {
    let mut a = RefreshPolicy::new(2);
    let b = a;
    let _ = a.next_refresh();
    assert_eq!(a.partials_since_full(), 1);
    assert_eq!(b.partials_since_full(), 0);
}
