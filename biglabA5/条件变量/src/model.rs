pub type Tick = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LockId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CondId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockedOn {
    Lock(LockId),
    Cond(CondId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Runnable,
    Blocked { on: BlockedOn },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Acquire(LockId),
    Release(LockId),
    CondWait { cond: CondId, lock: LockId },
    Signal { cond: CondId, lock: LockId },
    Broadcast { cond: CondId, lock: LockId },
    Hold(Tick),
    Work(Tick),
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: TaskId,
    pub actions: Vec<Action>,
    pub pc: usize,
    pub remaining: Tick,
    pub state: TaskState,
    pub cond_waiting: Option<(CondId, LockId)>,
}

impl Task {
    pub fn new(id: TaskId, actions: Vec<Action>) -> Self {
        Self {
            id,
            actions,
            pc: 0,
            remaining: 0,
            state: TaskState::Runnable,
            cond_waiting: None,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == TaskState::Done
    }

    pub fn current_action(&self) -> Option<Action> {
        self.actions.get(self.pc).copied()
    }

    pub fn advance(&mut self) {
        self.pc += 1;
        self.remaining = 0;
        if self.pc >= self.actions.len() {
            self.state = TaskState::Done;
        }
    }
}
