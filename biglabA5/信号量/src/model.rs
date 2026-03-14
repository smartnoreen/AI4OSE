pub type Tick = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TaskId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SemId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    Runnable,
    Blocked { on: SemId },
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Wait(SemId),
    Post(SemId),
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
