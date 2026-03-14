pub mod model;
pub mod primitives;
pub mod sim;

pub use model::{Action, SemId, TaskId, Tick};
pub use primitives::{SemaphoreKind, SleepSemBug, SpinSemBug};
pub use sim::{compare_spin_vs_sleep, ExperimentConfig, Metrics, RunError, Sim};
