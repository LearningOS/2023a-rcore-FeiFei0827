//! Process management syscalls
use crate::{
    config::{MAX_SYSCALL_NUM,PAGE_SIZE},
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, current_user_token,
    },
    timer::{get_time_us},
    mm::{get_physical},
    task::{get_task_info},
};

use crate::task::current_task_map;
use crate::task::current_task_unmap;

#[repr(C)]
#[derive(Debug)]
/// Time value
pub struct TimeVal {
    ///秒
    pub sec: usize,
    ///微秒
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
   pub status: TaskStatus,
    /// The numbers of syscall called by task
   pub  syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
   pub time: usize,
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

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    // trace!("kernel: sys_get_time");
    // -1
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let physical_address = get_physical(current_user_token(), _ts as usize);
    // println!("physical_address: {:p}", &physical_address);
    // println!("Virtual Address {:p}", _ts);
    // //physical_address: 0xfffffffffffe9ea0
    // //Virtual Address 0xbf00
    let ptr = physical_address as *mut TimeVal;
    unsafe {
        *ptr = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.(MMU)
/// HINT: What if [`TaskInfo`] is splitted by two pages ? 【不考虑这个问题】
/// ci-user\user\src\bin\ch3_taskinfo.rs
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {

    trace!("kernel: sys_task_info");
    // this is ch4
    // unsafe{
    //     *_ti = get_task_info();
    // }

    let physical_address = get_physical(current_user_token(), _ti as usize);
    let ptr = physical_address as *mut TaskInfo;
    unsafe{
        *ptr = get_task_info();
    }
    
    0
}

/// YOUR JOB: Implement mmap.
//
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    
    // check port
    if _port & !0x7 != 0 || _port & 0x7 == 0 || _start % PAGE_SIZE != 0{
        return -1;
    }


    // kernel_mmap(begin_virtual_addr, end_virtual_addr, _port);
    if _start % PAGE_SIZE != 0 {
        -1
    } else {
        current_task_map(_start, _len, _port)
    }

}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    // trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    // -1
    
    trace!("kernel: sys_munmap");

    // if _start % PAGE_SIZE != 0 {
    //     return -1;
    // } 

   
    if _start % PAGE_SIZE != 0 {
        -1
    } else {
        current_task_unmap(_start, _len)
    }
    
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
