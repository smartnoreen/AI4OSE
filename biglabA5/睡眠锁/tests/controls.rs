use sleep_lock_lab::model::{Action, LockId, Task, TaskId};
use sleep_lock_lab::primitives::{LockKind, SleepBug, SpinBug};
use sleep_lock_lab::sim::{RunError, Sim};

#[test]
fn control_spin_unlock_does_not_release_times_out() {
    let lock = LockKind::Spin(
        sleep_lock_lab::primitives::SpinLock::new(LockId(0)).with_bug(SpinBug {
            unlock_does_not_release: true,
            acquire_can_succeed_without_ownership: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::Acquire(LockId(0)),
                Action::Hold(3),
                Action::Release(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Acquire(LockId(0)), Action::Hold(1), Action::Release(LockId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Timeout { .. }));
}

#[test]
fn control_spin_acquire_without_ownership_is_caught() {
    let lock = LockKind::Spin(
        sleep_lock_lab::primitives::SpinLock::new(LockId(0)).with_bug(SpinBug {
            unlock_does_not_release: false,
            acquire_can_succeed_without_ownership: true,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::Acquire(LockId(0)),
                Action::Hold(10),
                Action::Release(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::Work(1),
                Action::Acquire(LockId(0)),
                Action::Hold(1),
                Action::Release(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::LockOwnershipViolation { .. }));
}

#[test]
fn control_sleep_no_wakeup_deadlocks() {
    let lock = LockKind::Sleep(
        sleep_lock_lab::primitives::SleepLock::new(LockId(0)).with_bug(SleepBug {
            no_wakeup: true,
            wake_before_release: false,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::Acquire(LockId(0)),
                Action::Hold(2),
                Action::Release(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Acquire(LockId(0)), Action::Hold(1), Action::Release(LockId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock));
}

#[test]
fn control_sleep_wake_before_release_reblocks_and_deadlocks() {
    let lock = LockKind::Sleep(
        sleep_lock_lab::primitives::SleepLock::new(LockId(0)).with_bug(SleepBug {
            no_wakeup: false,
            wake_before_release: true,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::Acquire(LockId(0)),
                Action::Hold(3),
                Action::Release(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Acquire(LockId(0)), Action::Hold(1), Action::Release(LockId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock | RunError::Timeout { .. }));
}
