pub mod model;
pub mod primitives;
pub mod sim;

pub use model::{Action, LockId, TaskId, Tick};
pub use primitives::{LockKind, SleepBug, SpinBug};
pub use sim::{compare_spin_vs_sleep, ExperimentConfig, Metrics, RunError, Sim};
