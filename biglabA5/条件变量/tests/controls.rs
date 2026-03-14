use condvar_lab::model::{Action, CondId, LockId, Task, TaskId};
use condvar_lab::primitives::{CondVar, CondVarBug, LockKind, SleepBug, SpinBug};
use condvar_lab::sim::{RunError, Sim};

#[test]
fn control_spin_unlock_does_not_release_times_out() {
    let lock = LockKind::Spin(
        condvar_lab::primitives::SpinLock::new(LockId(0)).with_bug(SpinBug {
            unlock_does_not_release: true,
            acquire_can_succeed_without_ownership: false,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Acquire(LockId(0)), Action::Hold(10), Action::Release(LockId(0))],
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
    let mut sim = Sim::new(tasks, vec![lock], vec![], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Timeout { .. }));
}

#[test]
fn control_sleep_no_wakeup_deadlocks() {
    let lock = LockKind::Sleep(
        condvar_lab::primitives::SleepLock::new(LockId(0)).with_bug(SleepBug {
            no_wakeup: true,
            wake_before_release: false,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Acquire(LockId(0)), Action::Hold(5), Action::Release(LockId(0))],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Acquire(LockId(0)), Action::Hold(1), Action::Release(LockId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], vec![], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock));
}

#[test]
fn control_condvar_signal_does_not_wake_deadlocks() {
    let lock = LockKind::Sleep(condvar_lab::primitives::SleepLock::new(LockId(0)));
    let cond = CondVar::new(CondId(0)).with_bug(CondVarBug {
        wait_does_not_release_lock: false,
        signal_does_not_wake: true,
        signal_does_not_mark_woken: false,
        broadcast_does_not_wake: false,
    });
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![
                Action::Acquire(LockId(0)),
                Action::CondWait {
                    cond: CondId(0),
                    lock: LockId(0),
                },
                Action::Hold(1),
                Action::Release(LockId(0)),
            ],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::Work(2),
                Action::Acquire(LockId(0)),
                Action::Signal {
                    cond: CondId(0),
                    lock: LockId(0),
                },
                Action::Release(LockId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![lock], vec![cond], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock | RunError::Timeout { .. }));
}
