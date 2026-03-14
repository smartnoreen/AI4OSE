use crate::model::{SemId, TaskId, Tick};
use crate::sim::{RunError, SemMetrics};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Default)]
pub struct SpinSemBug {
    pub post_does_not_increase: bool,
    pub wait_can_succeed_without_permit: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SleepSemBug {
    pub no_wakeup: bool,
    pub wake_before_grant: bool,
    pub wait_can_succeed_without_permit: bool,
}

#[derive(Clone, Debug)]
pub enum SemaphoreKind {
    Spin(SpinSemaphore),
    Sleep(SleepSemaphore),
}

#[derive(Clone, Debug)]
pub struct SpinSemaphore {
    pub id: SemId,
    pub permits: u64,
    pub max_permits: u64,
    pub holds: HashMap<TaskId, Tick>,
    pub bug: SpinSemBug,
}

#[derive(Clone, Debug)]
pub struct SleepSemaphore {
    pub id: SemId,
    pub permits: u64,
    pub max_permits: u64,
    pub holds: HashMap<TaskId, Tick>,
    pub waitq: VecDeque<TaskId>,
    pub bug: SleepSemBug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitResult {
    Acquired,
    AlreadyHeld,
    FailedSpin,
    BlockedSleep,
}

#[derive(Clone, Debug)]
pub struct PostResult {
    pub woken: Vec<TaskId>,
    pub granted: Vec<TaskId>,
}

impl PostResult {
    pub fn none() -> Self {
        Self {
            woken: Vec::new(),
            granted: Vec::new(),
        }
    }
}

impl SpinSemaphore {
    pub fn new(id: SemId, permits: u64) -> Self {
        Self {
            id,
            permits,
            max_permits: permits,
            holds: HashMap::new(),
            bug: SpinSemBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SpinSemBug) -> Self {
        self.bug = bug;
        self
    }

    pub fn is_holding(&self, task: TaskId) -> bool {
        self.holds.contains_key(&task)
    }

    pub fn try_wait(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<WaitResult, RunError> {
        if self.holds.contains_key(&task) {
            return Ok(WaitResult::AlreadyHeld);
        }
        if self.permits > 0 {
            self.permits -= 1;
            self.holds.insert(task, now);
            metrics.acquisitions += 1;
            Ok(WaitResult::Acquired)
        } else {
            metrics.contentions += 1;
            if self.bug.wait_can_succeed_without_permit {
                metrics.acquisitions += 1;
                Ok(WaitResult::Acquired)
            } else {
                Ok(WaitResult::FailedSpin)
            }
        }
    }

    pub fn post(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<PostResult, RunError> {
        let Some(start) = self.holds.remove(&task) else {
            return Err(RunError::BadPost { sem: self.id, task });
        };
        metrics.hold_time_total = metrics.hold_time_total.saturating_add(now.saturating_sub(start));
        if !self.bug.post_does_not_increase {
            if self.permits >= self.max_permits {
                return Err(RunError::PermitOverflow { sem: self.id, permits: self.permits });
            }
            self.permits += 1;
        }
        Ok(PostResult::none())
    }
}

impl SleepSemaphore {
    pub fn new(id: SemId, permits: u64) -> Self {
        Self {
            id,
            permits,
            max_permits: permits,
            holds: HashMap::new(),
            waitq: VecDeque::new(),
            bug: SleepSemBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SleepSemBug) -> Self {
        self.bug = bug;
        self
    }

    pub fn is_holding(&self, task: TaskId) -> bool {
        self.holds.contains_key(&task)
    }

    pub fn try_wait(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<WaitResult, RunError> {
        if self.holds.contains_key(&task) {
            return Ok(WaitResult::AlreadyHeld);
        }
        if self.permits > 0 {
            self.permits -= 1;
            self.holds.insert(task, now);
            metrics.acquisitions += 1;
            Ok(WaitResult::Acquired)
        } else {
            metrics.contentions += 1;
            if self.bug.wait_can_succeed_without_permit {
                metrics.acquisitions += 1;
                Ok(WaitResult::Acquired)
            } else {
                if !self.waitq.contains(&task) {
                    self.waitq.push_back(task);
                }
                Ok(WaitResult::BlockedSleep)
            }
        }
    }

    pub fn post(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<PostResult, RunError> {
        let Some(start) = self.holds.remove(&task) else {
            return Err(RunError::BadPost { sem: self.id, task });
        };
        metrics.hold_time_total = metrics.hold_time_total.saturating_add(now.saturating_sub(start));

        let mut res = PostResult::none();

        if self.bug.wake_before_grant {
            if !self.bug.no_wakeup {
                if let Some(next) = self.waitq.pop_front() {
                    res.woken.push(next);
                }
            }
            return Ok(res);
        }

        if self.bug.no_wakeup {
            if self.permits >= self.max_permits {
                return Err(RunError::PermitOverflow { sem: self.id, permits: self.permits });
            }
            self.permits += 1;
            return Ok(res);
        }

        if let Some(next) = self.waitq.pop_front() {
            self.holds.insert(next, now);
            metrics.acquisitions += 1;
            res.granted.push(next);
            res.woken.push(next);
            Ok(res)
        } else {
            if self.permits >= self.max_permits {
                return Err(RunError::PermitOverflow { sem: self.id, permits: self.permits });
            }
            self.permits += 1;
            Ok(res)
        }
    }
}

impl SemaphoreKind {
    pub fn id(&self) -> SemId {
        match self {
            SemaphoreKind::Spin(s) => s.id,
            SemaphoreKind::Sleep(s) => s.id,
        }
    }

    pub fn is_holding(&self, task: TaskId) -> bool {
        match self {
            SemaphoreKind::Spin(s) => s.is_holding(task),
            SemaphoreKind::Sleep(s) => s.is_holding(task),
        }
    }

    pub fn try_wait(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<WaitResult, RunError> {
        match self {
            SemaphoreKind::Spin(s) => s.try_wait(task, now, metrics),
            SemaphoreKind::Sleep(s) => s.try_wait(task, now, metrics),
        }
    }

    pub fn post(&mut self, task: TaskId, now: Tick, metrics: &mut SemMetrics) -> Result<PostResult, RunError> {
        match self {
            SemaphoreKind::Spin(s) => s.post(task, now, metrics),
            SemaphoreKind::Sleep(s) => s.post(task, now, metrics),
        }
    }
}
