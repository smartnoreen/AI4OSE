#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LockId(pub usize);

pub type Tick = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Runnable,
    Blocked { on: LockId },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    AcquireRead(LockId),
    ReleaseRead(LockId),
    AcquireWrite(LockId),
    ReleaseWrite(LockId),
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
}

impl Task {
    pub fn new(id: TaskId, actions: Vec<Action>) -> Self {
        Self {
            id,
            actions,
            pc: 0,
            remaining: 0,
            state: TaskState::Runnable,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == TaskState::Done || self.pc >= self.actions.len()
    }

    pub fn current_action(&self) -> Option<Action> {
        self.actions.get(self.pc).copied()
    }

    pub fn advance(&mut self) {
        self.pc = self.pc.saturating_add(1);
        if self.pc >= self.actions.len() {
            self.state = TaskState::Done;
        }
        self.remaining = 0;
    }
}
