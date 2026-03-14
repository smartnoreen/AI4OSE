use crate::model::{Action, SemId, Task, TaskId, TaskState, Tick};
use crate::primitives::{SemaphoreKind, WaitResult};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunError {
    Deadlock,
    Timeout { ticks: Tick },
    BadPost { sem: SemId, task: TaskId },
    PermitOverflow { sem: SemId, permits: u64 },
    SemaphoreGrantViolation { sem: SemId, task: TaskId },
}

#[derive(Clone, Debug, Default)]
pub struct SemMetrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub max_wait: Tick,
    pub starvation: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub contentions: u64,
    pub acquisitions: u64,
    pub hold_time_total: Tick,
    pub context_switches: u64,
    pub max_wait: Tick,
    pub starvation: bool,
    pub per_sem: Vec<SemMetrics>,
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
    pub num_tasks: usize,
    pub iterations: usize,
    pub permits: u64,
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
            permits: 1,
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
    sems: Vec<SemaphoreKind>,
    last_run: Option<TaskId>,
    wait_start: HashMap<(TaskId, SemId), Tick>,
    starvation_threshold: Tick,
    max_ticks: Tick,
    metrics: Metrics,
}

impl Sim {
    pub fn new(tasks: Vec<Task>, sems: Vec<SemaphoreKind>, max_ticks: Tick, starvation_threshold: Tick) -> Self {
        let runnable = tasks
            .iter()
            .filter(|t| t.state == TaskState::Runnable)
            .map(|t| t.id)
            .collect::<VecDeque<_>>();
        let per_sem = vec![SemMetrics::default(); sems.len()];
        Self {
            tick: 0,
            tasks,
            runnable,
            sems,
            last_run: None,
            wait_start: HashMap::new(),
            starvation_threshold,
            max_ticks,
            metrics: Metrics {
                per_sem,
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
            Action::Wait(sem_id) => {
                let sem_idx = sem_id.0;
                let per_sem = &mut self.metrics.per_sem[sem_idx];
                let res = self.sems[sem_idx].try_wait(id, self.tick, per_sem)?;
                match res {
                    WaitResult::Acquired | WaitResult::AlreadyHeld => {
                        if !self.sems[sem_idx].is_holding(id) {
                            return Err(RunError::SemaphoreGrantViolation { sem: sem_id, task: id });
                        }
                        self.on_acquired(id, sem_id)?;
                        self.tasks[idx].advance();
                        Ok(true)
                    }
                    WaitResult::FailedSpin => {
                        self.on_contended(id, sem_id);
                        Ok(true)
                    }
                    WaitResult::BlockedSleep => {
                        self.on_contended(id, sem_id);
                        self.tasks[idx].state = TaskState::Blocked { on: sem_id };
                        Ok(false)
                    }
                }
            }
            Action::Post(sem_id) => {
                let sem_idx = sem_id.0;
                let per_sem = &mut self.metrics.per_sem[sem_idx];
                let post = self.sems[sem_idx].post(id, self.tick, per_sem)?;
                for granted in post.granted {
                    self.on_acquired(granted, sem_id)?;
                }
                for woken in post.woken {
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

    fn on_contended(&mut self, task: TaskId, sem: SemId) {
        self.wait_start.entry((task, sem)).or_insert(self.tick);
    }

    fn on_acquired(&mut self, task: TaskId, sem: SemId) -> Result<(), RunError> {
        let Some(start) = self.wait_start.remove(&(task, sem)) else {
            return Ok(());
        };
        let waited = self.tick.saturating_sub(start);
        let sm = &mut self.metrics.per_sem[sem.0];
        if waited > sm.max_wait {
            sm.max_wait = waited;
        }
        if waited > self.metrics.max_wait {
            self.metrics.max_wait = waited;
        }
        if waited >= self.starvation_threshold {
            sm.starvation = true;
            self.metrics.starvation = true;
        }
        Ok(())
    }

    fn finalize_metrics(&mut self) {
        self.metrics.contentions = self.metrics.per_sem.iter().map(|m| m.contentions).sum();
        self.metrics.acquisitions = self.metrics.per_sem.iter().map(|m| m.acquisitions).sum();
        self.metrics.hold_time_total = self.metrics.per_sem.iter().map(|m| m.hold_time_total).sum();
        self.metrics.starvation = self.metrics.per_sem.iter().any(|m| m.starvation);
        self.metrics.max_wait = self.metrics.per_sem.iter().map(|m| m.max_wait).max().unwrap_or(0);
    }
}

pub fn compare_spin_vs_sleep(cfg: ExperimentConfig) -> Result<(Metrics, Metrics), RunError> {
    let spin = run_experiment(
        cfg,
        SemaphoreKind::Spin(crate::primitives::SpinSemaphore::new(SemId(0), cfg.permits)),
    );
    let sleep = run_experiment(
        cfg,
        SemaphoreKind::Sleep(crate::primitives::SleepSemaphore::new(SemId(0), cfg.permits)),
    );
    Ok((spin?, sleep?))
}

pub fn run_experiment(cfg: ExperimentConfig, sem: SemaphoreKind) -> Result<Metrics, RunError> {
    let mut rng = XorShift64::new(cfg.seed);
    let mut tasks = Vec::with_capacity(cfg.num_tasks);
    for i in 0..cfg.num_tasks {
        let mut actions = Vec::new();
        for _ in 0..cfg.iterations {
            actions.push(Action::Wait(SemId(0)));
            actions.push(Action::Hold(rng.gen_range(cfg.hold_min, cfg.hold_max)));
            actions.push(Action::Post(SemId(0)));
            actions.push(Action::Work(rng.gen_range(cfg.outside_min, cfg.outside_max)));
        }
        tasks.push(Task::new(TaskId(i), actions));
    }
    let mut sim = Sim::new(tasks, vec![sem], cfg.max_ticks, cfg.starvation_threshold);
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
