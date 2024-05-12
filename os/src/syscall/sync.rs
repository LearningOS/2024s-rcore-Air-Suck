use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
#[no_mangle]
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.detect_enable{
        // get current thread's num
        let thr_num=process_inner.thread_count();
        // get the resource's num
        let lock_num=process_inner.mutex_list.len();
        // initialize the vec
        let mut need=vec![vec![0usize; lock_num]; thr_num];
        let mut available=vec![0usize; lock_num];
        let mut allocation=vec![vec![0usize; lock_num]; thr_num];
        let mut finish=vec![false; thr_num];
        let current_tid = current_task().unwrap().get_tid();
        // initialize the available
        for i in 0..lock_num{
            let lock = process_inner.mutex_list[i].as_ref().unwrap();
            if lock.get_owner().is_none(){
                available[i]=1 as usize;
            }
        }
        // calculate the need(目前先认为所有阻塞的线程都需要一个资源，但实际上应该不是这样的)
        for i in 0..thr_num{
            // get the tid
            let tid1:usize;
            if let Some(thread) = process_inner.tasks[i].as_ref(){
                tid1=thread.get_tid();
                if tid1 == 999{
                    continue;
                }
            }else{
                return -1;
            }
            for j in 0..lock_num{
                // initialize the allocation and need
                if let Some(lock) = process_inner.mutex_list[j].as_ref(){
                    // initialize the need
                    lock.get_wait_queue().iter().for_each(|task|{
                        let tid2=task.get_tid();
                        if tid2==tid1{
                            need[i][j]+=1;
                        }
                    });
                    // initialize the allocation(the mutex is locked by the thread)
                    if let Some(task) = lock.get_owner() {
                        let tid2=task.get_tid();
                        if tid2==tid1{
                            allocation[i][j]+=1;
                        }
                    };
                    // current task need the resource
                    if tid1==current_tid && j==mutex_id{
                        need[i][j]+=1;
                    }
                }else{
                    return -1;
                }
            }
        }
        // initialize the work
        let mut work=available.clone();
        // try to detect the deadlock
        loop {
            let mut is_deadlock=true;
            let mut pass = true;
            // 一次只尝试修改一个finish成员
            for i in 0..thr_num{
                if finish[i]==false{
                    let mut can_alloc=true;
                    for j in 0..lock_num{
                        if need[i][j]>work[j]{
                            can_alloc=false;
                            break;
                        }
                    }
                    if can_alloc{
                        finish[i]=true;
                        is_deadlock=false;
                        for j in 0..lock_num{
                            work[j]+=allocation[i][j];
                        }
                        break;
                    }
                }
            }
            // check whether all threads are finished
            for i in 0..thr_num{
                if finish[i] == false{
                    pass=false;
                    break;
                }
            }
            // system is safe
            if pass {
                break;
            }
            // deadlock detected
            if is_deadlock{
                return -0xDEAD;
            }
        }
        let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
        drop(process_inner);
        mutex.lock();
        0
    }else{
        let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
        drop(process_inner);
        mutex.lock();
        0
    }
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.detect_enable{
        // get current thread's num
        let thr_num=process_inner.thread_count();
        // get the resource's num
        let res_num=process_inner.semaphore_list.len();
        // initialize the vec
        let mut need=vec![vec![0usize; res_num]; thr_num];
        let mut available=vec![0usize; res_num];
        let mut allocation=vec![vec![0usize; res_num]; thr_num];
        let mut finish=vec![false; thr_num];
        let current_tid = current_task().unwrap().get_tid();
        // println!("");
        // print the current_tid
        // println!("current_tid:{}, sem_id is {}", current_tid,sem_id);
        // initialize the available
        for  i in 0..res_num{
            let source = process_inner.semaphore_list[i].as_ref().unwrap();
            let source_num=source.inner.exclusive_access().count;
            // get the avliable num
            if source_num>0{
                available[i]=source_num as usize;
            }
        }
        // calculate the need(目前先认为所有阻塞的线程都需要一个资源，但实际上应该不是这样的)
        for i in 0 .. thr_num{
            // get the tid
            let tid1:usize;
            if let Some(thread) = process_inner.tasks[i].as_ref(){
                // get the tid
                tid1=thread.get_tid();
                if tid1 == 999{
                    continue;
                }
            }else{
                return -1;
            }
            for j in 0..res_num{
                // initialize the allocation and need
                if let Some(source) = process_inner.semaphore_list[j].as_ref(){
                    // initialize the need
                    source.inner.exclusive_access().wait_queue.iter().for_each(|task|{
                        let tid2=task.get_tid();
                        if tid2==tid1{
                            need[i][j]+=1;
                        }
                    });
                    // initialize the allocation
                    source.inner.exclusive_access().alloc_queue.iter().for_each(|task|{
                        let tid2=task.get_tid();
                        if tid2==tid1{
                            allocation[i][j]+=1;
                        }
                    });
                    // current task need the resource
                    if tid1==current_tid && j==sem_id{
                        need[i][j]+=1;
                    }
                }
            }
        }
        // print the allocation
        // println!("allocation:");
        // for i in 0..thr_num{
        //     for j in 0..res_num{
        //         print!("{} ", allocation[i][j]);
        //     }
        //     println!("");
        // }
        // initialize the work
        let mut work=available.clone();
        // try to detect the deadlock
        loop {
            let mut is_deadlock=true;
            let mut pass = true;
            // 一次只尝试修改一个finish成员
            for i in 0..thr_num{
                if finish[i]==false{
                    let mut can_alloc=true;
                    for j in 0..res_num{
                        if need[i][j]>work[j]{
                            can_alloc=false;
                            break;
                        }
                    }
                    if can_alloc{
                        finish[i]=true;
                        is_deadlock=false;
                        for j in 0..res_num{
                            work[j]+=allocation[i][j];
                        }
                        break;
                    }
                }
            }
            for i in 0..thr_num{
                if finish[i]==false{
                    pass=false;
                    break;
                }
            }
            // system is safe
            if pass {
                break;
            }
            // deadlock detected
            if is_deadlock{
                return -0xDEAD;
            }
        }
        let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
        drop(process_inner);
        sem.down();
        0
    }else{
        let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
        drop(process_inner);
        sem.down();
        0
    }
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    // get the current process
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    // charge wheather to enable deadlock detection
    if _enabled==1{
        process_inner.detect_enable=true;
    }else{
        process_inner.detect_enable=false;
    }
    drop(process_inner);
    0
}
