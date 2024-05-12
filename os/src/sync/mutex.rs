//! Mutex (spin-like and blocking(sleep))

use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use crate::task::{current_task, wakeup_task};
use alloc::{collections::VecDeque, sync::Arc};
use alloc::vec::Vec;
/// Mutex trait
pub trait Mutex: Sync + Send {
    /// Lock the mutex
    fn lock(&self);
    /// Unlock the mutex
    fn unlock(&self);
    /// get the wait queue
    fn get_wait_queue(&self) -> Vec<Arc<TaskControlBlock>>;
    /// get mutex owner
    fn get_owner(&self) -> Option<Arc<TaskControlBlock>>;
}

/// Spinlock Mutex struct
pub struct MutexSpin {
    inner: UPSafeCell<MutexSpinInner>,
}

pub struct MutexSpinInner{
    locked: bool,
    get_lock_tcb: Option<Arc<TaskControlBlock>>,
    wait_queue: Vec<Arc<TaskControlBlock>>,
}

impl MutexSpin {
    /// Create a new spinlock mutex
    pub fn new() -> Self {
        Self {
            inner: unsafe { UPSafeCell::new(
                MutexSpinInner{
                    locked: false,
                    get_lock_tcb: None,
                    wait_queue: Vec::new()
            })},
        }
    }
}

impl Mutex for MutexSpin {
    /// Lock the spinlock mutex
    fn lock(&self) {
        trace!("kernel: MutexSpin::lock");
        loop {
            let mut inner = self.inner.exclusive_access();
            if inner.locked {
                let mut need_push=true;
                // push current task to wait queue
                for task in inner.wait_queue.iter() {
                    // if current task is already in wait queue, no need to push again
                    if task.get_tid() == current_task().unwrap().get_tid() {
                        need_push=false;
                        break;
                    }
                }
                if need_push {
                    inner.wait_queue.push(current_task().unwrap());
                }
                drop(inner);
                suspend_current_and_run_next();
                continue;
            } else {
                inner.locked = true;
                inner.get_lock_tcb=current_task();
                drop(inner);
                return;
            }
        }
    }

    fn unlock(&self) {
        trace!("kernel: MutexSpin::unlock");
        let mut inner = self.inner.exclusive_access();
        inner.locked = false;
        inner.get_lock_tcb=None;
    }
    // get the wait queue
    fn get_wait_queue(&self) -> Vec<Arc<TaskControlBlock>> {
        let inner = self.inner.exclusive_access();
        inner.wait_queue.clone()
    }
    //get mutex owner
    fn get_owner(&self) -> Option<Arc<TaskControlBlock>> {
        let inner = self.inner.exclusive_access();
        inner.get_lock_tcb.clone()
    }
}

/// Blocking Mutex struct
pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
    get_lock_tcb: Option<Arc<TaskControlBlock>>,
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
                    get_lock_tcb: None,
                })
            },
        }
    }
}

impl Mutex for MutexBlocking {
    /// lock the blocking mutex
    fn lock(&self) {
        trace!("kernel: MutexBlocking::lock");
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            block_current_and_run_next();
        } else {
            mutex_inner.get_lock_tcb=current_task();
            mutex_inner.locked = true;
        }
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
        }
    }
    // get the wait queue
    fn get_wait_queue(&self) -> Vec<Arc<TaskControlBlock>> {
        let mutex_inner = self.inner.exclusive_access();
        let vec=mutex_inner.wait_queue.clone().into();
        vec
    }
    // get mutex owner
    fn get_owner(&self) -> Option<Arc<TaskControlBlock>> {
        let mutex_inner = self.inner.exclusive_access();
        mutex_inner.get_lock_tcb.clone()
    }
}
