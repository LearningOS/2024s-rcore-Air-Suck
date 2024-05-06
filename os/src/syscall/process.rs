//! Process management syscalls
use crate::mm::{PageTable, VirtAddr, VirtPageNum,MapPermission};
use crate::{
    config::{
        MAX_SYSCALL_NUM,PAGE_SIZE,
    },
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus,get_current_tcb,
    },
};

use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

#[no_mangle]
/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    // _ts is a virtual address
    trace!("kernel: sys_get_time");

    let tcb_raw_ptr = get_current_tcb();    //get the current task control block
    let page_table:PageTable;
    unsafe{
        page_table = PageTable::from_token((*tcb_raw_ptr).memory_set.token());  //get the page table
    }
    let us = get_time_us();     //get the time in us
    let va=VirtAddr::from(_ts as usize);    //get the virtual address
    let offset=va.page_offset();    //get the offset in the page
    let size =core::mem::size_of::<TimeVal>();
    // let buffers = translated_byte_buffer(current_user_token(), buf, len);   //get the translated byte buffer
    if offset+size>=PAGE_SIZE{   // TimeVal is splitted by two pages
        let vpn1=va.floor(); //get the virtual page number
        let vpn2=va.ceil();
        let ppn1=page_table.translate(vpn1).unwrap().ppn();   //get the physical page number
        let ppn2=page_table.translate(vpn2).unwrap().ppn();
        let ts_ptr1=((usize::from(ppn1)<<12)+offset) as *mut usize;   //get the pointer of TimeVal
        let ts_ptr2=(usize::from(ppn2)<<12) as *mut usize;
        unsafe {
            (*ts_ptr1) = us / 1_000_000; //the address must be word aligned
            (*ts_ptr2) = us % 1_000_000;
        }
    }else {
        // TimeVal is not splitted by two pages
        let vpn=va.floor(); //get the virtual page number
        let ppn=page_table.translate(vpn).unwrap().ppn();   //get the physical page number
        let ts_ptr=((usize::from(ppn)<<12)+offset) as *mut TimeVal;   //get the pointer of TimeVal
        unsafe {
            (*ts_ptr) = TimeVal{
                sec: us / 1_000_000,
                usec: us % 1_000_000,
            };
        }
    }
    0
}


/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    let tcb_raw_ptr = get_current_tcb();    //get the current task control block
    let page_table:PageTable;
    unsafe{
        page_table = PageTable::from_token((*tcb_raw_ptr).memory_set.token());  //get the page table
    }

    let us = get_time_us();     //get the time in us
    let va=VirtAddr::from(_ti as usize);    //get the virtual address
    let offset=va.page_offset();    //get the offset in the page
    //size_off返回的是字节数
    if va.page_offset()+core::mem::size_of::<TaskInfo>()>=PAGE_SIZE{   // TimeVal is splitted by two pages
        let vpn1=va.floor(); //get the virtual page number
        let vpn2=va.ceil();
        let ppn1=page_table.translate(vpn1).unwrap().ppn();   //get the physical page number
        let ppn2=page_table.translate(vpn2).unwrap().ppn();
        let mut temp_info:TaskInfo;
        unsafe{
            temp_info=TaskInfo{
                status:TaskStatus::Running,
                syscall_times:(*tcb_raw_ptr).syscall_times.clone(),
                time:(us-(*tcb_raw_ptr).sche_time.unwrap())/1000,
            };
        }
        // 接下来需要搬运TaskInfo，可以参考i之前load用户程序函数的实现
        // have to use raw pointer because of the demand of bit operation
        let info_raw_ptr=&mut temp_info as *const TaskInfo as *const u8;
        let info_slice: &[u8];
        let ts_ptr1=((usize::from(ppn1)<<12)+offset) as *mut u8;   //get the pointer of TimeVal
        let ts_ptr1_slice: &mut [u8];
        let ts_ptr2=(usize::from(ppn2)<<12) as *mut u8;
        let ts_ptr2_slice:&mut [u8];
        unsafe{
            info_slice =core::slice::from_raw_parts(info_raw_ptr,core::mem::size_of::<TaskInfo>());
            ts_ptr1_slice =core::slice::from_raw_parts_mut(ts_ptr1,PAGE_SIZE-offset);
            ts_ptr2_slice =core::slice::from_raw_parts_mut(ts_ptr2,core::mem::size_of::<TaskInfo>()-PAGE_SIZE+offset);
            // copy the first part of TaskInfo
            ts_ptr1_slice.copy_from_slice(&info_slice[..PAGE_SIZE-offset]);
            ts_ptr2_slice.copy_from_slice(&info_slice[PAGE_SIZE-offset..]);
        }
    }else {
        // TimeVal is not splitted by two pages
        let vpn=va.floor(); //get the virtual page number
        let ppn=page_table.translate(vpn).unwrap().ppn();   //get the physical page number
        let ts_ptr=((usize::from(ppn)<<12)+offset) as *mut TaskInfo;   //get the pointer of TimeVal
        unsafe{
            (*ts_ptr).status=TaskStatus::Running;
            (*ts_ptr).syscall_times=(*tcb_raw_ptr).syscall_times.clone();
            (*ts_ptr).time=(us-(*tcb_raw_ptr).sche_time.unwrap())/1000;
        }
    }
    0
}

#[no_mangle]
// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    if (_start%4096)!=0{                //start address is not page align
        println!("start address is not page align");
        return -1;
    }
    //assert the port is valid
    if _port&(!0x7)!=0||_port&0x7==0{
        println!("the port is not valid");
        return -1;
    }

    let vpn =VirtPageNum::from(_start>>12); //vpn is the virtual page number(page align)
    
    let page_num =(_len+4095)/4096;      //page_num is the number of pages
    
    let tcb_raw_ptr = get_current_tcb();    //get the current task control block
    let page_table: PageTable;
    unsafe{
        page_table = PageTable::from_token((*tcb_raw_ptr).memory_set.token());  //get the page table
    }

    //check the page_entry is valid or not
    for i in 0..page_num{
        match page_table.translate(VirtPageNum::from(usize::from(vpn)+i)){
            Some(pte) if pte.is_valid()=>{
                return -1;
            }
            _=>{continue;}
        }
    }
    //create a new map
    unsafe{
        //need to crate a new MapArea
        let mut flags =MapPermission::from_bits((_port<<1) as u8).unwrap();  //shift the port to the left by 1(为了对上页表项的标志位)
        flags=flags | MapPermission::U;     // add the user flag
        (*tcb_raw_ptr).memory_set.insert_framed_area(VirtAddr::from(_start),VirtAddr::from(_start+_len),flags);  //insert the new map area
        (*tcb_raw_ptr).memory_set.append_to(VirtAddr::from(_start),VirtAddr::from(_start+_len));    //init the new map area
    }
    return 0;
}

#[no_mangle]
// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    
    if (_start%4096)!=0{                //start address is not page align
        println!("start address is not page align");
        return -1;
    }

    let vpn =VirtPageNum::from(_start>>12); //vpn is the virtual page number(page align)

    let page_num =(_len+4095)/4096;      //page_num is the number of pages

    let tcb_raw_ptr = get_current_tcb();    //get the current task control block
    let page_table:PageTable;
    unsafe{
        page_table = PageTable::from_token((*tcb_raw_ptr).memory_set.token());  //get the page table
    }
    //check the pageentry is valid or not
    for i in 0..page_num{
        match page_table.translate(VirtPageNum::from(usize::from(vpn)+i)){
            Some(pte) if pte.is_valid()=>{continue;}
            _=>{
                return -1;
            }
        }
    }
    
    unsafe{
        (*tcb_raw_ptr).memory_set.shrink_to(VirtAddr::from(_start),VirtAddr::from(_start)); //ummap the map area
    }
    return 0;
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
