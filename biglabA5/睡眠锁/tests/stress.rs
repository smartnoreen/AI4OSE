use sleep_lock_lab::primitives::LockKind;
use sleep_lock_lab::sim::{run_experiment, ExperimentConfig};

#[test]
fn stress_correct_versions_pass_and_are_observable() {
    let cfg = ExperimentConfig {
        num_tasks: 12,
        iterations: 100,
        hold_min: 1,
        hold_max: 7,
        outside_min: 0,
        outside_max: 2,
        max_ticks: 2_000_000,
        starvation_threshold: 200_000,
        seed: 12345,
    };

    let spin = run_experiment(cfg, LockKind::Spin(sleep_lock_lab::primitives::SpinLock::new(
        sleep_lock_lab::LockId(0),
    )))
    .unwrap();

    let sleep = run_experiment(cfg, LockKind::Sleep(sleep_lock_lab::primitives::SleepLock::new(
        sleep_lock_lab::LockId(0),
    )))
    .unwrap();

    assert!(spin.contentions > 0);
    assert!(sleep.contentions > 0);
    assert!(!spin.starvation);
    assert!(!sleep.starvation);
    assert!(spin.acquisitions > 0);
    assert!(sleep.acquisitions > 0);
    assert!(spin.avg_hold_time() > 0.0);
    assert!(sleep.avg_hold_time() > 0.0);
}

#[test]
fn sleep_lock_has_fewer_context_switches_in_high_contention() {
    let cfg = ExperimentConfig {
        num_tasks: 8,
        iterations: 20,
        hold_min: 10,
        hold_max: 10,
        outside_min: 0,
        outside_max: 0,
        max_ticks: 500_000,
        starvation_threshold: 100_000,
        seed: 7,
    };

    let spin = run_experiment(cfg, LockKind::Spin(sleep_lock_lab::primitives::SpinLock::new(
        sleep_lock_lab::LockId(0),
    )))
    .unwrap();

    let sleep = run_experiment(cfg, LockKind::Sleep(sleep_lock_lab::primitives::SleepLock::new(
        sleep_lock_lab::LockId(0),
    )))
    .unwrap();

    assert!(spin.context_switches > sleep.context_switches);
    assert!(spin.max_wait > 0);
    assert!(sleep.max_wait > 0);
}
