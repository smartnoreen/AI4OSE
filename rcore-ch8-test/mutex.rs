//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};

/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self) -> bool;
    /// Unlock the mutex
    fn unlock(&self);
    /// Check if current task is the owner
    fn is_locked_by_current(&self) -> bool;
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    locked: UPSafeCell<bool>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) -> bool {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return true;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }

    fn is_locked_by_current(&self) -> bool {
        let locked = self.locked.exclusive_access();
        *locked
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    owner: Option<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    /// Create a new blocking mutex
    pub fn new() -> Self {
        trace!("kernel: MutexBlocking::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                    owner: None,
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) -> bool {
        trace!("kernel: MutexBlocking::lock");
        let current = current_task().unwrap();
        let mut mutex_inner = self.inner.exclusive_access();

        // Check if current task already owns this mutex (deadlock)
        if let Some(ref owner) = mutex_inner.owner {
            if Arc::ptr_eq(owner, &current) {
                // Self-deadlock detected
                return false;
            }
        }

        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current);
            drop(mutex_inner);
            block_current_and_run_next();
            // After wakeup, acquire the lock
            let mut mutex_inner = self.inner.exclusive_access();
            mutex_inner.owner = Some(current_task().unwrap());
        } else {
            mutex_inner.locked = true;
            mutex_inner.owner = Some(current);
        }
        true
    }

    /// unlock the blocking mutex
    fn unlock(&self) {
        trace!("kernel: MutexBlocking::unlock");
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            wakeup_task(waking_task);
        } else {
            mutex_inner.locked = false;
            mutex_inner.owner = None;
        }
    }

    fn is_locked_by_current(&self) -> bool {
        let mutex_inner = self.inner.exclusive_access();
        if let Some(ref owner) = mutex_inner.owner {
            Arc::ptr_eq(owner, &current_task().unwrap())
        } else {
            false
        }
    }
}
