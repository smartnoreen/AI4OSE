use crate::model::{CondId, LockId, TaskId, Tick};
use crate::sim::{CondMetrics, LockMetrics, RunError};
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Copy, Debug, Default)]
pub struct SpinBug {
    pub unlock_does_not_release: bool,
    pub acquire_can_succeed_without_ownership: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SleepBug {
    pub no_wakeup: bool,
    pub wake_before_release: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CondVarBug {
    pub wait_does_not_release_lock: bool,
    pub signal_does_not_wake: bool,
    pub signal_does_not_mark_woken: bool,
    pub broadcast_does_not_wake: bool,
}

#[derive(Clone, Debug)]
pub enum LockKind {
    Spin(SpinLock),
    Sleep(SleepLock),
}

#[derive(Clone, Debug)]
pub struct SpinLock {
    pub id: LockId,
    pub holder: Option<TaskId>,
    pub acquired_at: Option<Tick>,
    pub bug: SpinBug,
}

#[derive(Clone, Debug)]
pub struct SleepLock {
    pub id: LockId,
    pub holder: Option<TaskId>,
    pub acquired_at: Option<Tick>,
    pub waitq: VecDeque<TaskId>,
    pub bug: SleepBug,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AcquireResult {
    Acquired,
    AlreadyHeld,
    FailedSpin,
    BlockedSleep,
}

#[derive(Clone, Debug)]
pub struct ReleaseResult {
    pub woken: Vec<TaskId>,
    pub granted: Vec<TaskId>,
}

impl ReleaseResult {
    pub fn none() -> Self {
        Self {
            woken: Vec::new(),
            granted: Vec::new(),
        }
    }
}

impl SpinLock {
    pub fn new(id: LockId) -> Self {
        Self {
            id,
            holder: None,
            acquired_at: None,
            bug: SpinBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SpinBug) -> Self {
        self.bug = bug;
        self
    }

    pub fn try_acquire(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<AcquireResult, RunError> {
        match self.holder {
            None => {
                self.holder = Some(task);
                self.acquired_at = Some(now);
                metrics.acquisitions += 1;
                Ok(AcquireResult::Acquired)
            }
            Some(t) if t == task => Ok(AcquireResult::AlreadyHeld),
            Some(_) => {
                metrics.contentions += 1;
                if self.bug.acquire_can_succeed_without_ownership {
                    metrics.acquisitions += 1;
                    Ok(AcquireResult::Acquired)
                } else {
                    Ok(AcquireResult::FailedSpin)
                }
            }
        }
    }

    pub fn release(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<ReleaseResult, RunError> {
        if self.holder != Some(task) {
            return Err(RunError::BadUnlock { lock: self.id, task });
        }
        if !self.bug.unlock_does_not_release {
            self.holder = None;
        }
        if let Some(start) = self.acquired_at.take() {
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(now.saturating_sub(start));
        }
        Ok(ReleaseResult::none())
    }
}

impl SleepLock {
    pub fn new(id: LockId) -> Self {
        Self {
            id,
            holder: None,
            acquired_at: None,
            waitq: VecDeque::new(),
            bug: SleepBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SleepBug) -> Self {
        self.bug = bug;
        self
    }

    pub fn try_acquire(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<AcquireResult, RunError> {
        match self.holder {
            None => {
                self.holder = Some(task);
                self.acquired_at = Some(now);
                metrics.acquisitions += 1;
                Ok(AcquireResult::Acquired)
            }
            Some(t) if t == task => Ok(AcquireResult::AlreadyHeld),
            Some(_) => {
                metrics.contentions += 1;
                if !self.waitq.contains(&task) {
                    self.waitq.push_back(task);
                }
                Ok(AcquireResult::BlockedSleep)
            }
        }
    }

    pub fn release(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<ReleaseResult, RunError> {
        if self.holder != Some(task) {
            return Err(RunError::BadUnlock { lock: self.id, task });
        }

        let mut res = ReleaseResult::none();

        if self.bug.wake_before_release {
            if !self.bug.no_wakeup {
                if let Some(next) = self.waitq.pop_front() {
                    res.woken.push(next);
                }
            }
            return Ok(res);
        }

        self.holder = None;

        if let Some(start) = self.acquired_at.take() {
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(now.saturating_sub(start));
        }

        if !self.bug.no_wakeup {
            if let Some(next) = self.waitq.pop_front() {
                self.holder = Some(next);
                self.acquired_at = Some(now);
                metrics.acquisitions += 1;
                res.granted.push(next);
                res.woken.push(next);
            }
        }

        Ok(res)
    }
}

impl LockKind {
    pub fn try_acquire(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<AcquireResult, RunError> {
        match self {
            LockKind::Spin(l) => l.try_acquire(task, now, metrics),
            LockKind::Sleep(l) => l.try_acquire(task, now, metrics),
        }
    }

    pub fn release(
        &mut self,
        task: TaskId,
        now: Tick,
        metrics: &mut LockMetrics,
    ) -> Result<ReleaseResult, RunError> {
        match self {
            LockKind::Spin(l) => l.release(task, now, metrics),
            LockKind::Sleep(l) => l.release(task, now, metrics),
        }
    }

    pub fn holder(&self) -> Option<TaskId> {
        match self {
            LockKind::Spin(l) => l.holder,
            LockKind::Sleep(l) => l.holder,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CondVar {
    pub id: CondId,
    waitq: VecDeque<TaskId>,
    woken: HashSet<TaskId>,
    pub bug: CondVarBug,
}

impl CondVar {
    pub fn new(id: CondId) -> Self {
        Self {
            id,
            waitq: VecDeque::new(),
            woken: HashSet::new(),
            bug: CondVarBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: CondVarBug) -> Self {
        self.bug = bug;
        self
    }

    pub fn begin_wait(
        &mut self,
        task: TaskId,
        _lock: LockId,
        now: Tick,
        lock_obj: &mut LockKind,
        lock_metrics: &mut LockMetrics,
    ) -> Result<ReleaseResult, RunError> {
        if !self.waitq.contains(&task) {
            self.waitq.push_back(task);
        }
        if self.bug.wait_does_not_release_lock {
            Ok(ReleaseResult::none())
        } else {
            lock_obj.release(task, now, lock_metrics)
        }
    }

    pub fn signal(&mut self) -> Vec<TaskId> {
        if self.bug.signal_does_not_wake {
            return Vec::new();
        }
        let Some(t) = self.waitq.pop_front() else {
            return Vec::new();
        };
        if !self.bug.signal_does_not_mark_woken {
            self.woken.insert(t);
        }
        vec![t]
    }

    pub fn broadcast(&mut self) -> Vec<TaskId> {
        if self.bug.broadcast_does_not_wake {
            return Vec::new();
        }
        let mut res = Vec::new();
        while let Some(t) = self.waitq.pop_front() {
            self.woken.insert(t);
            res.push(t);
        }
        res
    }

    pub fn is_woken(&self, task: TaskId) -> bool {
        self.woken.contains(&task)
    }

    pub fn finish_wait(&mut self, task: TaskId, _metrics: &mut CondMetrics) {
        self.woken.remove(&task);
    }
}
