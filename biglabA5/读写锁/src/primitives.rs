use crate::model::{LockId, TaskId, Tick};
use crate::sim::{RwLockMetrics, RunError};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Default)]
pub struct SpinRwBug {
    pub unlock_write_does_not_release: bool,
    pub unlock_read_does_not_release: bool,
    pub acquire_can_succeed_even_if_conflict: bool,
    pub reader_barge_while_writer_pending: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SleepRwBug {
    pub no_wakeup: bool,
    pub wake_before_release: bool,
    pub reader_barge_while_writer_waiting: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TryResult {
    Acquired,
    AlreadyHeld,
    FailedSpin,
    BlockedSleep,
}

#[derive(Clone, Debug)]
pub struct ReleaseResult {
    pub woken: Vec<TaskId>,
    pub granted_read: Vec<TaskId>,
    pub granted_write: Vec<TaskId>,
}

impl ReleaseResult {
    pub fn none() -> Self {
        Self {
            woken: Vec::new(),
            granted_read: Vec::new(),
            granted_write: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitMode {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Waiter {
    task: TaskId,
    mode: WaitMode,
}

#[derive(Clone, Debug)]
pub enum RwLockKind {
    Spin(SpinRwLock),
    Sleep(SleepRwLock),
}

#[derive(Clone, Debug)]
pub struct SpinRwLock {
    pub id: LockId,
    pub writer: Option<TaskId>,
    pub readers: Vec<TaskId>,
    pub acquired_at_write: Option<Tick>,
    pub acquired_at_read: HashMap<TaskId, Tick>,
    pub writer_pending: bool,
    pub bug: SpinRwBug,
}

#[derive(Clone, Debug)]
pub struct SleepRwLock {
    pub id: LockId,
    pub writer: Option<TaskId>,
    pub readers: Vec<TaskId>,
    pub acquired_at_write: Option<Tick>,
    pub acquired_at_read: HashMap<TaskId, Tick>,
    waitq: VecDeque<Waiter>,
    pub bug: SleepRwBug,
}

impl SpinRwLock {
    pub fn new(id: LockId) -> Self {
        Self {
            id,
            writer: None,
            readers: Vec::new(),
            acquired_at_write: None,
            acquired_at_read: HashMap::new(),
            writer_pending: false,
            bug: SpinRwBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SpinRwBug) -> Self {
        self.bug = bug;
        self
    }

    fn holds_read(&self, task: TaskId) -> bool {
        self.readers.contains(&task)
    }

    fn holds_write(&self, task: TaskId) -> bool {
        self.writer == Some(task)
    }

    pub fn try_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        if self.holds_read(task) {
            return Ok(TryResult::AlreadyHeld);
        }
        if self.holds_write(task) {
            return Ok(TryResult::AlreadyHeld);
        }

        let conflict = self.writer.is_some()
            || (self.writer_pending && !self.bug.reader_barge_while_writer_pending);
        if conflict {
            metrics.contentions += 1;
            if self.bug.acquire_can_succeed_even_if_conflict {
                self.readers.push(task);
                self.acquired_at_read.insert(task, now);
                metrics.read_acquisitions += 1;
                metrics.acquisitions += 1;
                Ok(TryResult::Acquired)
            } else {
                Ok(TryResult::FailedSpin)
            }
        } else {
            self.readers.push(task);
            self.acquired_at_read.insert(task, now);
            metrics.read_acquisitions += 1;
            metrics.acquisitions += 1;
            Ok(TryResult::Acquired)
        }
    }

    pub fn try_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        if self.holds_write(task) {
            return Ok(TryResult::AlreadyHeld);
        }
        if self.holds_read(task) {
            metrics.contentions += 1;
            self.writer_pending = true;
            return Ok(TryResult::FailedSpin);
        }

        if self.writer.is_none() && self.readers.is_empty() {
            self.writer = Some(task);
            self.acquired_at_write = Some(now);
            self.writer_pending = false;
            metrics.write_acquisitions += 1;
            metrics.acquisitions += 1;
            Ok(TryResult::Acquired)
        } else {
            metrics.contentions += 1;
            self.writer_pending = true;
            if self.bug.acquire_can_succeed_even_if_conflict {
                self.writer = Some(task);
                self.acquired_at_write = Some(now);
                self.writer_pending = false;
                metrics.write_acquisitions += 1;
                metrics.acquisitions += 1;
                Ok(TryResult::Acquired)
            } else {
                Ok(TryResult::FailedSpin)
            }
        }
    }

    pub fn release_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        if !self.holds_read(task) {
            return Err(RunError::BadUnlockRead { lock: self.id, task });
        }

        if !self.bug.unlock_read_does_not_release {
            self.readers.retain(|t| *t != task);
        }

        if let Some(start) = self.acquired_at_read.remove(&task) {
            let dt = now.saturating_sub(start);
            metrics.read_hold_time_total = metrics.read_hold_time_total.saturating_add(dt);
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(dt);
        }

        Ok(ReleaseResult::none())
    }

    pub fn release_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        if self.writer != Some(task) {
            return Err(RunError::BadUnlockWrite { lock: self.id, task });
        }

        if !self.bug.unlock_write_does_not_release {
            self.writer = None;
        }

        if let Some(start) = self.acquired_at_write.take() {
            let dt = now.saturating_sub(start);
            metrics.write_hold_time_total = metrics.write_hold_time_total.saturating_add(dt);
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(dt);
        }

        Ok(ReleaseResult::none())
    }
}

impl SleepRwLock {
    pub fn new(id: LockId) -> Self {
        Self {
            id,
            writer: None,
            readers: Vec::new(),
            acquired_at_write: None,
            acquired_at_read: HashMap::new(),
            waitq: VecDeque::new(),
            bug: SleepRwBug::default(),
        }
    }

    pub fn with_bug(mut self, bug: SleepRwBug) -> Self {
        self.bug = bug;
        self
    }

    fn holds_read(&self, task: TaskId) -> bool {
        self.readers.contains(&task)
    }

    fn holds_write(&self, task: TaskId) -> bool {
        self.writer == Some(task)
    }

    fn writer_waiting(&self) -> bool {
        self.waitq.iter().any(|w| w.mode == WaitMode::Write)
    }

    fn enqueue(&mut self, task: TaskId, mode: WaitMode) {
        if self.waitq.iter().any(|w| w.task == task && w.mode == mode) {
            return;
        }
        self.waitq.push_back(Waiter { task, mode });
    }

    pub fn try_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        if self.holds_read(task) {
            return Ok(TryResult::AlreadyHeld);
        }
        if self.holds_write(task) {
            return Ok(TryResult::AlreadyHeld);
        }

        let blocked_by_writer = self.writer.is_some();
        let blocked_by_waiting_writer = self.writer_waiting() && !self.bug.reader_barge_while_writer_waiting;
        if blocked_by_writer || blocked_by_waiting_writer {
            metrics.contentions += 1;
            self.enqueue(task, WaitMode::Read);
            Ok(TryResult::BlockedSleep)
        } else {
            self.readers.push(task);
            self.acquired_at_read.insert(task, now);
            metrics.read_acquisitions += 1;
            metrics.acquisitions += 1;
            Ok(TryResult::Acquired)
        }
    }

    pub fn try_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        if self.holds_write(task) {
            return Ok(TryResult::AlreadyHeld);
        }
        if self.holds_read(task) {
            metrics.contentions += 1;
            self.enqueue(task, WaitMode::Write);
            return Ok(TryResult::BlockedSleep);
        }

        if self.writer.is_none() && self.readers.is_empty() {
            self.writer = Some(task);
            self.acquired_at_write = Some(now);
            metrics.write_acquisitions += 1;
            metrics.acquisitions += 1;
            Ok(TryResult::Acquired)
        } else {
            metrics.contentions += 1;
            self.enqueue(task, WaitMode::Write);
            Ok(TryResult::BlockedSleep)
        }
    }

    fn grant_from_waitq(&mut self, now: Tick, metrics: &mut RwLockMetrics) -> ReleaseResult {
        let mut res = ReleaseResult::none();
        if self.writer.is_some() || !self.readers.is_empty() {
            return res;
        }

        let Some(front) = self.waitq.front().copied() else {
            return res;
        };

        match front.mode {
            WaitMode::Write => {
                let w = self.waitq.pop_front().unwrap();
                self.writer = Some(w.task);
                self.acquired_at_write = Some(now);
                metrics.write_acquisitions += 1;
                metrics.acquisitions += 1;
                res.granted_write.push(w.task);
                res.woken.push(w.task);
            }
            WaitMode::Read => {
                while let Some(w) = self.waitq.front().copied() {
                    if w.mode != WaitMode::Read {
                        break;
                    }
                    let w = self.waitq.pop_front().unwrap();
                    self.readers.push(w.task);
                    self.acquired_at_read.insert(w.task, now);
                    metrics.read_acquisitions += 1;
                    metrics.acquisitions += 1;
                    res.granted_read.push(w.task);
                    res.woken.push(w.task);
                }
            }
        }
        res
    }

    pub fn release_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        if !self.holds_read(task) {
            return Err(RunError::BadUnlockRead { lock: self.id, task });
        }

        let mut res = ReleaseResult::none();

        if self.bug.wake_before_release {
            if !self.bug.no_wakeup {
                if let Some(w) = self.waitq.pop_front() {
                    res.woken.push(w.task);
                }
            }
            return Ok(res);
        }

        self.readers.retain(|t| *t != task);
        if let Some(start) = self.acquired_at_read.remove(&task) {
            let dt = now.saturating_sub(start);
            metrics.read_hold_time_total = metrics.read_hold_time_total.saturating_add(dt);
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(dt);
        }

        if self.readers.is_empty() && !self.bug.no_wakeup {
            res = self.grant_from_waitq(now, metrics);
        }

        Ok(res)
    }

    pub fn release_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        if self.writer != Some(task) {
            return Err(RunError::BadUnlockWrite { lock: self.id, task });
        }

        let mut res = ReleaseResult::none();

        if self.bug.wake_before_release {
            if !self.bug.no_wakeup {
                if let Some(w) = self.waitq.pop_front() {
                    res.woken.push(w.task);
                }
            }
            return Ok(res);
        }

        self.writer = None;
        if let Some(start) = self.acquired_at_write.take() {
            let dt = now.saturating_sub(start);
            metrics.write_hold_time_total = metrics.write_hold_time_total.saturating_add(dt);
            metrics.hold_time_total = metrics.hold_time_total.saturating_add(dt);
        }

        if !self.bug.no_wakeup {
            res = self.grant_from_waitq(now, metrics);
        }
        Ok(res)
    }
}

impl RwLockKind {
    pub fn id(&self) -> LockId {
        match self {
            RwLockKind::Spin(l) => l.id,
            RwLockKind::Sleep(l) => l.id,
        }
    }

    pub fn writer(&self) -> Option<TaskId> {
        match self {
            RwLockKind::Spin(l) => l.writer,
            RwLockKind::Sleep(l) => l.writer,
        }
    }

    pub fn readers(&self) -> &[TaskId] {
        match self {
            RwLockKind::Spin(l) => &l.readers,
            RwLockKind::Sleep(l) => &l.readers,
        }
    }

    pub fn try_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        match self {
            RwLockKind::Spin(l) => l.try_read(task, now, metrics),
            RwLockKind::Sleep(l) => l.try_read(task, now, metrics),
        }
    }

    pub fn try_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<TryResult, RunError> {
        match self {
            RwLockKind::Spin(l) => l.try_write(task, now, metrics),
            RwLockKind::Sleep(l) => l.try_write(task, now, metrics),
        }
    }

    pub fn release_read(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        match self {
            RwLockKind::Spin(l) => l.release_read(task, now, metrics),
            RwLockKind::Sleep(l) => l.release_read(task, now, metrics),
        }
    }

    pub fn release_write(&mut self, task: TaskId, now: Tick, metrics: &mut RwLockMetrics) -> Result<ReleaseResult, RunError> {
        match self {
            RwLockKind::Spin(l) => l.release_write(task, now, metrics),
            RwLockKind::Sleep(l) => l.release_write(task, now, metrics),
        }
    }
}
