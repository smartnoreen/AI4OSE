use semaphore_lab::model::{Action, SemId, Task, TaskId};
use semaphore_lab::primitives::{SemaphoreKind, SleepSemBug, SpinSemBug};
use semaphore_lab::sim::{RunError, Sim};

#[test]
fn control_spin_post_does_not_increase_times_out() {
    let sem = SemaphoreKind::Spin(
        semaphore_lab::primitives::SpinSemaphore::new(SemId(0), 1).with_bug(SpinSemBug {
            post_does_not_increase: true,
            wait_can_succeed_without_permit: false,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Wait(SemId(0)), Action::Hold(3), Action::Post(SemId(0))],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Wait(SemId(0)), Action::Hold(1), Action::Post(SemId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![sem], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Timeout { .. }));
}

#[test]
fn control_spin_wait_without_permit_is_caught() {
    let sem = SemaphoreKind::Spin(
        semaphore_lab::primitives::SpinSemaphore::new(SemId(0), 1).with_bug(SpinSemBug {
            post_does_not_increase: false,
            wait_can_succeed_without_permit: true,
        }),
    );

    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Wait(SemId(0)), Action::Hold(10), Action::Post(SemId(0))],
        ),
        Task::new(
            TaskId(1),
            vec![
                Action::Work(1),
                Action::Wait(SemId(0)),
                Action::Hold(1),
                Action::Post(SemId(0)),
            ],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![sem], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::SemaphoreGrantViolation { .. }));
}

#[test]
fn control_sleep_no_wakeup_deadlocks() {
    let sem = SemaphoreKind::Sleep(
        semaphore_lab::primitives::SleepSemaphore::new(SemId(0), 1).with_bug(SleepSemBug {
            no_wakeup: true,
            wake_before_grant: false,
            wait_can_succeed_without_permit: false,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Wait(SemId(0)), Action::Hold(2), Action::Post(SemId(0))],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Wait(SemId(0)), Action::Hold(1), Action::Post(SemId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![sem], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock));
}

#[test]
fn control_sleep_wake_before_grant_reblocks_and_deadlocks() {
    let sem = SemaphoreKind::Sleep(
        semaphore_lab::primitives::SleepSemaphore::new(SemId(0), 1).with_bug(SleepSemBug {
            no_wakeup: false,
            wake_before_grant: true,
            wait_can_succeed_without_permit: false,
        }),
    );
    let tasks = vec![
        Task::new(
            TaskId(0),
            vec![Action::Wait(SemId(0)), Action::Hold(3), Action::Post(SemId(0))],
        ),
        Task::new(
            TaskId(1),
            vec![Action::Wait(SemId(0)), Action::Hold(1), Action::Post(SemId(0))],
        ),
    ];
    let mut sim = Sim::new(tasks, vec![sem], 200, 10_000);
    let err = sim.run().unwrap_err();
    assert!(matches!(err, RunError::Deadlock | RunError::Timeout { .. }));
}
