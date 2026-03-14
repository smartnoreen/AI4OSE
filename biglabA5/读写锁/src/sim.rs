use crate::model::{Action, LockId, Task, TaskId, TaskState, Tick};
use crate::primitives::{RwLockKind, TryResult};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    Deadlock,
    Timeout { ticks: Tick },
    BadUnlockRead { lock: LockId, task: TaskId },
    BadUnlockWrite { lock: LockId, task: TaskId },
    RwLockGrantViolation {
        lock: LockId,
        task: TaskId,
        writer: Option<TaskId>,
        readers: usize,
    },
}

#[derive(Clone, Debug, Default)]
pub struct RwLockMetrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub read_acquisitions: u64,
    pub write_acquisitions: u64,
    pub read_hold_time_total: Tick,
    pub write_hold_time_total: Tick,
    pub max_wait: Tick,
    pub max_read_wait: Tick,
    pub max_write_wait: Tick,
    pub starvation: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub read_acquisitions: u64,
    pub write_acquisitions: u64,
    pub read_hold_time_total: Tick,
    pub write_hold_time_total: Tick,
    pub context_switches: u64,
    pub max_wait: Tick,
    pub max_read_wait: Tick,
    pub max_write_wait: Tick,
    pub starvation: bool,
    pub per_lock: Vec<RwLockMetrics>,
}

impl Metrics {
    pub fn avg_hold_time(&self) -> f64 {
        if self.acquisitions == 0 {
            0.0
        } else {
            self.hold_time_total as f64 / self.acquisitions as f64
        }
    }

    pub fn avg_read_hold_time(&self) -> f64 {
        if self.read_acquisitions == 0 {
            0.0
        } else {
            self.read_hold_time_total as f64 / self.read_acquisitions as f64
        }
    }

    pub fn avg_write_hold_time(&self) -> f64 {
        if self.write_acquisitions == 0 {
            0.0
        } else {
            self.write_hold_time_total as f64 / self.write_acquisitions as f64
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ExperimentConfig {
    pub num_tasks: usize,
    pub iterations: usize,
    pub read_percent: u8,
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
            num_tasks: 8,
            iterations: 50,
            read_percent: 80,
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
    locks: Vec<RwLockKind>,
    last_run: Option<TaskId>,
    read_wait_start: HashMap<(TaskId, LockId), Tick>,
    write_wait_start: HashMap<(TaskId, LockId), Tick>,
    starvation_threshold: Tick,
    max_ticks: Tick,
    metrics: Metrics,
}

impl Sim {
    pub fn new(tasks: Vec<Task>, locks: Vec<RwLockKind>, max_ticks: Tick, starvation_threshold: Tick) -> Self {
        let runnable = tasks
            .iter()
            .filter(|t| t.state == TaskState::Runnable)
            .map(|t| t.id)
            .collect::<VecDeque<_>>();
        let per_lock = vec![RwLockMetrics::default(); locks.len()];
        Self {
            tick: 0,
            tasks,
            runnable,
            locks,
            last_run: None,
            read_wait_start: HashMap::new(),
            write_wait_start: HashMap::new(),
            starvation_threshold,
            max_ticks,
            metrics: Metrics {
                per_lock,
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
            Action::AcquireRead(lock_id) => {
                let lock_idx = lock_id.0;
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                let res = self.locks[lock_idx].try_read(id, self.tick, per_lock)?;
                match res {
                    TryResult::Acquired => {
                        let writer = self.locks[lock_idx].writer();
                        let readers = self.locks[lock_idx].readers();
                        if writer.is_some() || !readers.contains(&id) {
                            return Err(RunError::RwLockGrantViolation {
                                lock: lock_id,
                                task: id,
                                writer,
                                readers: readers.len(),
                            });
                        }
                        self.on_acquired_read(id, lock_id)?;
                        self.tasks[idx].advance();
                        Ok(true)
                    }
                    TryResult::AlreadyHeld => {
                        self.tasks[idx].advance();
                        Ok(true)
                    }
                    TryResult::FailedSpin => {
                        self.on_contended_read(id, lock_id);
                        Ok(true)
                    }
                    TryResult::BlockedSleep => {
                        self.on_contended_read(id, lock_id);
                        self.tasks[idx].state = TaskState::Blocked { on: lock_id };
                        Ok(false)
                    }
                }
            }
            Action::AcquireWrite(lock_id) => {
                let lock_idx = lock_id.0;
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                let res = self.locks[lock_idx].try_write(id, self.tick, per_lock)?;
                match res {
                    TryResult::Acquired => {
                        let writer = self.locks[lock_idx].writer();
                        let readers = self.locks[lock_idx].readers();
                        if writer != Some(id) || !readers.is_empty() {
                            return Err(RunError::RwLockGrantViolation {
                                lock: lock_id,
                                task: id,
                                writer,
                                readers: readers.len(),
                            });
                        }
                        self.on_acquired_write(id, lock_id)?;
                        self.tasks[idx].advance();
                        Ok(true)
                    }
                    TryResult::AlreadyHeld => {
                        self.tasks[idx].advance();
                        Ok(true)
                    }
                    TryResult::FailedSpin => {
                        self.on_contended_write(id, lock_id);
                        Ok(true)
                    }
                    TryResult::BlockedSleep => {
                        self.on_contended_write(id, lock_id);
                        self.tasks[idx].state = TaskState::Blocked { on: lock_id };
                        Ok(false)
                    }
                }
            }
            Action::ReleaseRead(lock_id) => {
                let lock_idx = lock_id.0;
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                let release = self.locks[lock_idx].release_read(id, self.tick, per_lock)?;
                for granted in release.granted_read {
                    self.on_acquired_read(granted, lock_id)?;
                }
                for granted in release.granted_write {
                    self.on_acquired_write(granted, lock_id)?;
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
            Action::ReleaseWrite(lock_id) => {
                let lock_idx = lock_id.0;
                let per_lock = &mut self.metrics.per_lock[lock_idx];
                let release = self.locks[lock_idx].release_write(id, self.tick, per_lock)?;
                for granted in release.granted_read {
                    self.on_acquired_read(granted, lock_id)?;
                }
                for granted in release.granted_write {
                    self.on_acquired_write(granted, lock_id)?;
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

    fn on_contended_read(&mut self, task: TaskId, lock: LockId) {
        self.read_wait_start.entry((task, lock)).or_insert(self.tick);
    }

    fn on_contended_write(&mut self, task: TaskId, lock: LockId) {
        self.write_wait_start.entry((task, lock)).or_insert(self.tick);
    }

    fn on_acquired_read(&mut self, task: TaskId, lock: LockId) -> Result<(), RunError> {
        let Some(start) = self.read_wait_start.remove(&(task, lock)) else {
            return Ok(());
        };
        let waited = self.tick.saturating_sub(start);
        let lm = &mut self.metrics.per_lock[lock.0];
        if waited > lm.max_read_wait {
            lm.max_read_wait = waited;
        }
        lm.max_wait = lm.max_wait.max(waited);
        if waited >= self.starvation_threshold {
            lm.starvation = true;
        }
        Ok(())
    }

    fn on_acquired_write(&mut self, task: TaskId, lock: LockId) -> Result<(), RunError> {
        let Some(start) = self.write_wait_start.remove(&(task, lock)) else {
            return Ok(());
        };
        let waited = self.tick.saturating_sub(start);
        let lm = &mut self.metrics.per_lock[lock.0];
        if waited > lm.max_write_wait {
            lm.max_write_wait = waited;
        }
        lm.max_wait = lm.max_wait.max(waited);
        if waited >= self.starvation_threshold {
            lm.starvation = true;
        }
        Ok(())
    }

    fn finalize_metrics(&mut self) {
        self.metrics.contentions = self.metrics.per_lock.iter().map(|m| m.contentions).sum();
        self.metrics.acquisitions = self.metrics.per_lock.iter().map(|m| m.acquisitions).sum();
        self.metrics.hold_time_total = self.metrics.per_lock.iter().map(|m| m.hold_time_total).sum();
        self.metrics.read_acquisitions = self.metrics.per_lock.iter().map(|m| m.read_acquisitions).sum();
        self.metrics.write_acquisitions = self.metrics.per_lock.iter().map(|m| m.write_acquisitions).sum();
        self.metrics.read_hold_time_total = self.metrics.per_lock.iter().map(|m| m.read_hold_time_total).sum();
        self.metrics.write_hold_time_total = self.metrics.per_lock.iter().map(|m| m.write_hold_time_total).sum();
        self.metrics.starvation = self.metrics.per_lock.iter().any(|m| m.starvation);
        self.metrics.max_wait = self.metrics.per_lock.iter().map(|m| m.max_wait).max().unwrap_or(0);
        self.metrics.max_read_wait = self.metrics.per_lock.iter().map(|m| m.max_read_wait).max().unwrap_or(0);
        self.metrics.max_write_wait = self.metrics.per_lock.iter().map(|m| m.max_write_wait).max().unwrap_or(0);
    }
}

pub fn compare_spin_vs_sleep(cfg: ExperimentConfig) -> Result<(Metrics, Metrics), RunError> {
    let spin = run_experiment(cfg, RwLockKind::Spin(crate::primitives::SpinRwLock::new(LockId(0))));
    let sleep = run_experiment(cfg, RwLockKind::Sleep(crate::primitives::SleepRwLock::new(LockId(0))));
    Ok((spin?, sleep?))
}

pub fn run_experiment(cfg: ExperimentConfig, lock: RwLockKind) -> Result<Metrics, RunError> {
    let mut rng = XorShift64::new(cfg.seed);
    let mut tasks = Vec::with_capacity(cfg.num_tasks);
    for i in 0..cfg.num_tasks {
        let mut actions = Vec::new();
        for _ in 0..cfg.iterations {
            if rng.gen_percent(cfg.read_percent) {
                actions.push(Action::AcquireRead(LockId(0)));
                actions.push(Action::Hold(rng.gen_range(cfg.hold_min, cfg.hold_max)));
                actions.push(Action::ReleaseRead(LockId(0)));
            } else {
                actions.push(Action::AcquireWrite(LockId(0)));
                actions.push(Action::Hold(rng.gen_range(cfg.hold_min, cfg.hold_max)));
                actions.push(Action::ReleaseWrite(LockId(0)));
            }
            actions.push(Action::Work(rng.gen_range(cfg.outside_min, cfg.outside_max)));
        }
        tasks.push(Task::new(TaskId(i), actions));
    }
    let mut sim = Sim::new(tasks, vec![lock], cfg.max_ticks, cfg.starvation_threshold);
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

    fn gen_percent(&mut self, percent: u8) -> bool {
        if percent >= 100 {
            return true;
        }
        let v = (self.next_u64() % 100) as u8;
        v < percent
    }
}
