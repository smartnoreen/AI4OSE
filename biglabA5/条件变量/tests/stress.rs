use condvar_lab::primitives::LockKind;
use condvar_lab::sim::{run_signal_experiment, ExperimentConfig};

#[test]
fn stress_correct_versions_pass_and_are_observable() {
    let cfg = ExperimentConfig {
        num_waiters: 12,
        iterations: 100,
        hold_min: 1,
        hold_max: 7,
        outside_min: 0,
        outside_max: 2,
        max_ticks: 3_000_000,
        starvation_threshold: 200_000,
        seed: 12345,
    };

    let spin = run_signal_experiment(
        cfg,
        LockKind::Spin(condvar_lab::primitives::SpinLock::new(condvar_lab::LockId(0))),
    )
    .unwrap();

    let sleep = run_signal_experiment(
        cfg,
        LockKind::Sleep(condvar_lab::primitives::SleepLock::new(condvar_lab::LockId(0))),
    )
    .unwrap();

    assert!(spin.contentions > 0);
    assert!(sleep.contentions > 0);
    assert!(spin.acquisitions > 0);
    assert!(sleep.acquisitions > 0);
    assert!(spin.avg_hold_time() > 0.0);
    assert!(sleep.avg_hold_time() > 0.0);
    assert!(!spin.starvation);
    assert!(!sleep.starvation);
    assert!(spin.max_lock_wait > 0);
    assert!(sleep.max_lock_wait > 0);
    assert!(spin.max_cond_wait > 0);
    assert!(sleep.max_cond_wait > 0);
    assert!(spin.per_cond[0].wakeups > 0);
    assert!(sleep.per_cond[0].wakeups > 0);
}

#[test]
fn spin_lock_has_more_context_switches_under_condvar_contention() {
    let cfg = ExperimentConfig {
        num_waiters: 8,
        iterations: 30,
        hold_min: 10,
        hold_max: 10,
        outside_min: 0,
        outside_max: 0,
        max_ticks: 2_000_000,
        starvation_threshold: 200_000,
        seed: 7,
    };

    let spin = run_signal_experiment(
        cfg,
        LockKind::Spin(condvar_lab::primitives::SpinLock::new(condvar_lab::LockId(0))),
    )
    .unwrap();

    let sleep = run_signal_experiment(
        cfg,
        LockKind::Sleep(condvar_lab::primitives::SleepLock::new(condvar_lab::LockId(0))),
    )
    .unwrap();

    assert!(spin.context_switches > sleep.context_switches);
    assert!(spin.max_wait > 0);
    assert!(sleep.max_wait > 0);
}
