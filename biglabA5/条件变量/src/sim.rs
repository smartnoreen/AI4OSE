use crate::model::{Action, BlockedOn, CondId, LockId, Task, TaskId, TaskState, Tick};
use crate::primitives::{AcquireResult, CondVar, LockKind};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    Deadlock,
    Timeout { ticks: Tick },
    BadUnlock { lock: LockId, task: TaskId },
    LockOwnershipViolation { lock: LockId, task: TaskId, holder: Option<TaskId> },
    CondWaitWithoutLock { cond: CondId, lock: LockId, task: TaskId },
    CondSignalWithoutLock { cond: CondId, lock: LockId, task: TaskId },
    CondWaitNotWoken { cond: CondId, task: TaskId },
}

#[derive(Clone, Debug, Default)]
pub struct LockMetrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub max_wait: Tick,
    pub starvation: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CondMetrics {
    pub waits: u64,
    pub signals: u64,
    pub broadcasts: u64,
    pub wakeups: u64,
    pub max_wait: Tick,
    pub starvation: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub context_switches: u64,
    pub max_lock_wait: Tick,
    pub max_cond_wait: Tick,
    pub max_wait: Tick,
    pub starvation: bool,
    pub per_lock: Vec<LockMetrics>,
    pub per_cond: Vec<CondMetrics>,
}

impl Metrics {
    pub fn avg_hold_time(&self) -> f64 {
        if self.acquisitions == 0 {
            0.0
        } else {
            self.hold_time_total as f64 / self.acquisitions as f64
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExperimentConfig {
    pub num_waiters: usize,
    pub iterations: usize,
    pub hold_min: Tick,
    pub hold_max: Tick,
    pub outside_min: Tick,
    pub outside_max: Tick,
    pub max_ticks: Tick,
    pub starvation_threshold: Tick,
    pub seed: u64,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            num_waiters: 8,
            iterations: 50,
            hold_min: 1,
            hold_max: 5,
            outside_min: 0,
            outside_max: 3,
            max_ticks: 1_000_000,
            starvation_threshold: 50_000,
            seed: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sim {
    pub tick: Tick,
    tasks: Vec<Task>,
    runnable: VecDeque<TaskId>,
    locks: Vec<LockKind>,
    conds: Vec<CondVar>,
    last_run: Option<TaskId>,
    lock_wait_start: HashMap<(TaskId, LockId), Tick>,
    cond_wait_start: HashMap<(TaskId, CondId), Tick>,
    starvation_threshold: Tick,
    max_ticks: Tick,
    metrics: Metrics,
}

impl Sim {
    pub fn new(
        tasks: Vec<Task>,
        locks: Vec<LockKind>,
        conds: Vec<CondVar>,
        max_ticks: Tick,
        starvation_threshold: Tick,
    ) -> Self {
        let runnable = tasks
            .iter()
            .filter(|t| t.state == TaskState::Runnable)
            .map(|t| t.id)
            .collect::<VecDeque<_>>();
        let per_lock = vec![LockMetrics::default(); locks.len()];
        let per_cond = vec![CondMetrics::default(); conds.len()];
        Self {
            tick: 0,
            tasks,
            runnable,
            locks,
            conds,
            last_run: None,
            lock_wait_start: HashMap::new(),
            cond_wait_start: HashMap::new(),
            starvation_threshold,
            max_ticks,
            metrics: Metrics {
                per_lock,
                per_cond,
                ..Metrics::default()
            },
        }
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    pub fn run(&mut self) -> Result<Metrics, RunError> {
        while !self.tasks.iter().all(|t| t.is_done()) {
            if self.tick >= self.max_ticks {
                return Err(RunError::Timeout { ticks: self.tick });
            }
            let task_id = match self.runnable.pop_front() {
                Some(t) => t,
                None => return Err(RunError::Deadlock),
            };
            if self.last_run != Some(task_id) {
                if self.last_run.is_some() {
                    self.metrics.context_switches += 1;
                }
                self.last_run = Some(task_id);
            }

            let push_back = self.step_one(task_id)?;
            if push_back {
                self.runnable.push_back(task_id);
            }
            self.tick = self.tick.saturating_add(1);
        }
        self.finalize_metrics();
        Ok(self.metrics.clone())
    }

    fn task_index(&self, id: TaskId) -> usize {
        id.0
    }

    fn step_one(&mut self, id: TaskId) -> Result<bool, RunError> {
        let idx = self.task_index(id);
        if self.tasks[idx].state != TaskState::Runnable {
            return Ok(false);
        }
        let Some(action) = self.tasks[idx].current_action() else {
            self.tasks[idx].state = TaskState::Done;
            return Ok(false);
        };

        match action {
            Action::Acquire(lock_id) => self.step_acquire(idx, id, lock_id),
            Action::Release(lock_id) => self.step_release(idx, id, lock_id),
            Action::CondWait { cond, lock } => self.step_cond_wait(idx, id, cond, lock),
            Action::Signal { cond, lock } => self.step_signal(idx, id, cond, lock, false),
            Action::Broadcast { cond, lock } => self.step_signal(idx, id, cond, lock, true),
            Action::Hold(n) | Action::Work(n) => {
                if self.tasks[idx].remaining == 0 {
                    self.tasks[idx].remaining = n;
                }
                if self.tasks[idx].remaining > 0 {
                    self.tasks[idx].remaining -= 1;
                }
                if self.tasks[idx].remaining == 0 {
                    self.tasks[idx].advance();
                }
                Ok(true)
            }
        }
    }

    fn step_acquire(&mut self, idx: usize, id: TaskId, lock_id: LockId) -> Result<bool, RunError> {
        let lock_idx = lock_id.0;
        let holder = self.locks[lock_idx].holder();
        let per_lock = &mut self.metrics.per_lock[lock_idx];
        let res = self.locks[lock_idx].try_acquire(id, self.tick, per_lock)?;
        match res {
            AcquireResult::Acquired => {
                if self.locks[lock_idx].holder() != Some(id) {
                    return Err(RunError::LockOwnershipViolation {
                        lock: lock_id,
                        task: id,
                        holder,
                    });
                }
                self.on_lock_acquired(id, lock_id)?;
                self.tasks[idx].advance();
                Ok(true)
            }
            AcquireResult::AlreadyHeld => {
                if self.locks[lock_idx].holder() != Some(id) {
                    return Err(RunError::LockOwnershipViolation {
                        lock: lock_id,
                        task: id,
                        holder,
                    });
                }
                self.tasks[idx].advance();
                Ok(true)
            }
            AcquireResult::FailedSpin => {
                self.on_lock_contended(id, lock_id);
                Ok(true)
            }
            AcquireResult::BlockedSleep => {
                self.on_lock_contended(id, lock_id);
                self.tasks[idx].state = TaskState::Blocked {
                    on: BlockedOn::Lock(lock_id),
                };
                Ok(false)
            }
        }
    }

    fn step_release(&mut self, idx: usize, id: TaskId, lock_id: LockId) -> Result<bool, RunError> {
        let lock_idx = lock_id.0;
        let per_lock = &mut self.metrics.per_lock[lock_idx];
        let release = self.locks[lock_idx].release(id, self.tick, per_lock)?;
        for granted in release.granted {
            self.on_lock_acquired(granted, lock_id)?;
        }
        for woken in release.woken {
            let widx = self.task_index(woken);
            if self.tasks[widx].state != TaskState::Done {
                self.tasks[widx].state = TaskState::Runnable;
                self.runnable.push_back(woken);
            }
        }
        self.tasks[idx].advance();
        Ok(true)
    }

    fn step_cond_wait(
        &mut self,
        idx: usize,
        id: TaskId,
        cond: CondId,
        lock: LockId,
    ) -> Result<bool, RunError> {
        let lock_idx = lock.0;
        let cond_idx = cond.0;

        if self.tasks[idx].cond_waiting.is_none() {
            if self.locks[lock_idx].holder() != Some(id) {
                return Err(RunError::CondWaitWithoutLock { cond, lock, task: id });
            }
            self.metrics.per_cond[cond_idx].waits += 1;
            self.cond_wait_start.entry((id, cond)).or_insert(self.tick);
            let unlock = {
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                self.conds[cond_idx].begin_wait(id, lock, self.tick, &mut self.locks[lock_idx], per_lock)?
            };
            for granted in unlock.granted {
                self.on_lock_acquired(granted, lock)?;
            }
            for woken in unlock.woken {
                let widx = self.task_index(woken);
                if self.tasks[widx].state != TaskState::Done {
                    self.tasks[widx].state = TaskState::Runnable;
                    self.runnable.push_back(woken);
                }
            }
            self.tasks[idx].cond_waiting = Some((cond, lock));
            self.tasks[idx].state = TaskState::Blocked {
                on: BlockedOn::Cond(cond),
            };
            Ok(false)
        } else {
            if !self.conds[cond_idx].is_woken(id) {
                return Err(RunError::CondWaitNotWoken { cond, task: id });
            }
            let holder = self.locks[lock_idx].holder();
            let res = {
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                self.locks[lock_idx].try_acquire(id, self.tick, per_lock)?
            };
            match res {
                AcquireResult::Acquired | AcquireResult::AlreadyHeld => {
                    if self.locks[lock_idx].holder() != Some(id) {
                        return Err(RunError::LockOwnershipViolation {
                            lock,
                            task: id,
                            holder,
                        });
                    }
                    self.on_lock_acquired(id, lock)?;
                    self.on_cond_finished(id, cond)?;
                    {
                        let per_cond = &mut self.metrics.per_cond[cond_idx];
                        self.conds[cond_idx].finish_wait(id, per_cond);
                    }
                    self.tasks[idx].cond_waiting = None;
                    self.tasks[idx].advance();
                    Ok(true)
                }
                AcquireResult::FailedSpin => {
                    self.on_lock_contended(id, lock);
                    Ok(true)
                }
                AcquireResult::BlockedSleep => {
                    self.on_lock_contended(id, lock);
                    self.tasks[idx].state = TaskState::Blocked {
                        on: BlockedOn::Lock(lock),
                    };
                    Ok(false)
                }
            }
        }
    }

    fn step_signal(
        &mut self,
        idx: usize,
        id: TaskId,
        cond: CondId,
        lock: LockId,
        broadcast: bool,
    ) -> Result<bool, RunError> {
        let lock_idx = lock.0;
        let cond_idx = cond.0;
        if self.locks[lock_idx].holder() != Some(id) {
            return Err(RunError::CondSignalWithoutLock { cond, lock, task: id });
        }
        if broadcast {
            self.metrics.per_cond[cond_idx].broadcasts += 1;
        } else {
            self.metrics.per_cond[cond_idx].signals += 1;
        }
        let woken = if broadcast {
            self.conds[cond_idx].broadcast()
        } else {
            self.conds[cond_idx].signal()
        };
        self.metrics.per_cond[cond_idx].wakeups += woken.len() as u64;
        for t in woken {
            let widx = self.task_index(t);
            if self.tasks[widx].state != TaskState::Done {
                self.tasks[widx].state = TaskState::Runnable;
                self.runnable.push_back(t);
            }
        }
        self.tasks[idx].advance();
        Ok(true)
    }

    fn on_lock_contended(&mut self, task: TaskId, lock: LockId) {
        self.lock_wait_start.entry((task, lock)).or_insert(self.tick);
    }

    fn on_lock_acquired(&mut self, task: TaskId, lock: LockId) -> Result<(), RunError> {
        let Some(start) = self.lock_wait_start.remove(&(task, lock)) else {
            return Ok(());
        };
        let waited = self.tick.saturating_sub(start);
        let lm = &mut self.metrics.per_lock[lock.0];
        if waited > lm.max_wait {
            lm.max_wait = waited;
        }
        if waited > self.metrics.max_lock_wait {
            self.metrics.max_lock_wait = waited;
        }
        if waited >= self.starvation_threshold {
            lm.starvation = true;
            self.metrics.starvation = true;
        }
        Ok(())
    }

    fn on_cond_finished(&mut self, task: TaskId, cond: CondId) -> Result<(), RunError> {
        let Some(start) = self.cond_wait_start.remove(&(task, cond)) else {
            return Ok(());
        };
        let waited = self.tick.saturating_sub(start);
        let cm = &mut self.metrics.per_cond[cond.0];
        if waited > cm.max_wait {
            cm.max_wait = waited;
        }
        if waited > self.metrics.max_cond_wait {
            self.metrics.max_cond_wait = waited;
        }
        if waited >= self.starvation_threshold {
            cm.starvation = true;
            self.metrics.starvation = true;
        }
        Ok(())
    }

    fn finalize_metrics(&mut self) {
        self.metrics.contentions = self.metrics.per_lock.iter().map(|m| m.contentions).sum();
        self.metrics.acquisitions = self.metrics.per_lock.iter().map(|m| m.acquisitions).sum();
        self.metrics.hold_time_total = self.metrics.per_lock.iter().map(|m| m.hold_time_total).sum();
        self.metrics.starvation = self.metrics.per_lock.iter().any(|m| m.starvation)
            || self.metrics.per_cond.iter().any(|m| m.starvation);
        self.metrics.max_lock_wait = self.metrics.per_lock.iter().map(|m| m.max_wait).max().unwrap_or(0);
        self.metrics.max_cond_wait = self.metrics.per_cond.iter().map(|m| m.max_wait).max().unwrap_or(0);
        self.metrics.max_wait = self.metrics.max_lock_wait.max(self.metrics.max_cond_wait);
    }
}

pub fn compare_spin_vs_sleep(cfg: ExperimentConfig) -> Result<(Metrics, Metrics), RunError> {
    let spin = run_signal_experiment(cfg, LockKind::Spin(crate::primitives::SpinLock::new(LockId(0))));
    let sleep = run_signal_experiment(cfg, LockKind::Sleep(crate::primitives::SleepLock::new(LockId(0))));
    Ok((spin?, sleep?))
}

pub fn run_signal_experiment(cfg: ExperimentConfig, lock: LockKind) -> Result<Metrics, RunError> {
    let mut rng = XorShift64::new(cfg.seed);
    let lock_id = LockId(0);
    let cond_id = CondId(0);

    let mut tasks = Vec::with_capacity(cfg.num_waiters + 1);
    for i in 0..cfg.num_waiters {
        let mut actions = Vec::new();
        for _ in 0..cfg.iterations {
            actions.push(Action::Acquire(lock_id));
            actions.push(Action::CondWait { cond: cond_id, lock: lock_id });
            actions.push(Action::Hold(rng.gen_range(cfg.hold_min, cfg.hold_max)));
            actions.push(Action::Release(lock_id));
            actions.push(Action::Work(rng.gen_range(cfg.outside_min, cfg.outside_max)));
        }
        tasks.push(Task::new(TaskId(i), actions));
    }

    let mut signaler = Vec::new();
    signaler.push(Action::Work((cfg.num_waiters as Tick).saturating_mul(8)));
    for _ in 0..(cfg.num_waiters * cfg.iterations) {
        signaler.push(Action::Acquire(lock_id));
        signaler.push(Action::Signal { cond: cond_id, lock: lock_id });
        signaler.push(Action::Release(lock_id));
        signaler.push(Action::Work(rng.gen_range(cfg.outside_min, cfg.outside_max)));
    }
    tasks.push(Task::new(TaskId(cfg.num_waiters), signaler));

    let mut sim = Sim::new(
        tasks,
        vec![lock],
        vec![CondVar::new(cond_id)],
        cfg.max_ticks,
        cfg.starvation_threshold,
    );
    sim.run()
}

#[derive(Clone, Copy, Debug)]
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        let s = if seed == 0 { 0x9e3779b97f4a7c15 } else { seed };
        Self(s)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn gen_range(&mut self, min: Tick, max: Tick) -> Tick {
        if min >= max {
            return min;
        }
        let span = max - min + 1;
        min + (self.next_u64() % span as u64) as Tick
    }
}
