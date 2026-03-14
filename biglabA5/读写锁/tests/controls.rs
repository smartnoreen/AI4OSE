use rwlock_lab::model::{Action, LockId, Task, TaskId};
use rwlock_lab::primitives::{RwLockKind, SleepRwBug, SpinRwBug};
use rwlock_lab::sim::{RunError, Sim};

#[test]
fn control_spin_unlock_write_does_not_release_times_out() {
    let lock = RwLockKind::Spin(
        rwlock_lab::primitives::SpinRwLock::new(LockId(0)).with_bug(SpinRwBug {
            unlock_write_does_not_release: true,
            unlock_read_does_not_release: false,
            acquire_can_succeed_even_if_conflict: false,
            reader_barge_while_writer_pending: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(3),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(1),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Timeout { .. }));
}

#[test]
fn control_spin_acquire_even_if_conflict_is_caught() {
    let lock = RwLockKind::Spin(
        rwlock_lab::primitives::SpinRwLock::new(LockId(0)).with_bug(SpinRwBug {
            unlock_write_does_not_release: false,
            unlock_read_does_not_release: false,
            acquire_can_succeed_even_if_conflict: true,
            reader_barge_while_writer_pending: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(10),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::Work(1),
                Action::AcquireRead(LockId(0)),
                Action::Hold(1),
                Action::ReleaseRead(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::RwLockGrantViolation { .. }));
}

#[test]
fn control_sleep_no_wakeup_deadlocks() {
    let lock = RwLockKind::Sleep(
        rwlock_lab::primitives::SleepRwLock::new(LockId(0)).with_bug(SleepRwBug {
            no_wakeup: true,
            wake_before_release: false,
            reader_barge_while_writer_waiting: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(2),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(1),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock));
}

#[test]
fn control_sleep_wake_before_release_reblocks_and_deadlocks() {
    let lock = RwLockKind::Sleep(
        rwlock_lab::primitives::SleepRwLock::new(LockId(0)).with_bug(SleepRwBug {
            no_wakeup: false,
            wake_before_release: true,
            reader_barge_while_writer_waiting: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(3),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::AcquireWrite(LockId(0)),
                Action::Hold(1),
                Action::ReleaseWrite(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock | RunError::Timeout { .. }));
}
