//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::{get_app_data, get_num_app};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
use crate::config::MAX_SYSCALL_NUM;
use crate::timer::get_time_ms;
use crate::syscall::process::TaskInfo;
use crate::mm::VirtAddr;
use crate::mm::MapPermission;

/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
    /// add_syscall_count
    fn add_syscall_count(&self,sys_call_id : usize)
    {
        if sys_call_id >= MAX_SYSCALL_NUM {
            return;
        }
        let mut inner = self.inner.exclusive_access(); 
        let current = inner.current_task;
        inner.tasks[current].syscall_count[sys_call_id]+=1; //
    }

    ///获取当前running任务的信息
     /// 返回：//学到了
    ///impl TaskInfo {
    ///    pub fn new() -> Self {
    ///        TaskInfo {
    ///            status: TaskStatus::UnInit,
    ///            syscall_times: [0; MAX_SYSCALL_NUM],
    ///           time: 0,
    ///        }
    ///    }
     ///}
     pub fn get_current_task_info(&self) -> TaskInfo {
        let inner = self.inner.exclusive_access();
        let current_task_id = inner.current_task; 

        // task list tasks: [TaskControlBlock; MAX_APP_NUM],
        let current_task = &inner.tasks[current_task_id];
         
        let task_info = TaskInfo {
            status: current_task.task_status.clone(),
            syscall_times:current_task.syscall_count.clone(),
        
            time: get_time_ms() - current_task.start_time,
        };
        task_info
    }

     /// kernel_mmap
     pub fn kernel_mmap(&self, start_virt_add: VirtAddr, end_virt_add: VirtAddr, port: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current_process_id = inner.current_task;
        let curr_task_tcb = &mut inner.tasks[current_process_id];
        //未当前进程分配mmp地址
        // bitflags! {
        //     /// map permission corresponding to that in pte: `R W X U`
        //     pub struct MapPermission: u8 {
        //         ///Readable
        //         const R = 1 << 1;
        //         ///Writable
        //         const W = 1 << 2;
        //         ///Excutable
        //         const X = 1 << 3;
        //         ///Accessible in U mode
        //         const U = 1 << 4;
        //     }
        // }
        
        // allocate
        let mut map_perm = MapPermission::U;
        
        //3 
        if port & 0x1 != 0 {
            map_perm |= MapPermission::R;
        }
        if port & 0x2 != 0 {
            map_perm |= MapPermission::W;
        }
        if port & 0x3 != 0 {
            map_perm |= MapPermission::X;
        }


        curr_task_tcb.memory_set.insert_framed_area(start_virt_add, end_virt_add, map_perm);
        
        // Assume that no conflicts.
        let result: isize = 0; // 用具体的返回值示例
        result // 返回 result 或者根据实际操作返回相应的 isize 值
    }
    /// kernel_unmmap
    pub fn kernel_unmmap(&self, start: VirtAddr, end: VirtAddr) {
        let mut inner = self.inner.exclusive_access();
        let current_process_id = inner.current_task;
        let curr_task_tcb = &mut inner.tasks[current_process_id];
        // if !curr_task_tcb.memory_set.check_all_allocated(start, end) {
        //     return -1;
        // }
        curr_task_tcb.memory_set.delete_framed_area(start, end);
        
    }
    /// check_all_allocated
    pub fn check_all_allocated(&self, start: VirtAddr, end: VirtAddr) -> bool {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].check_all_allocated(start, end)
    }
    /// check_allocated
    fn check_allocated(&self, start: VirtAddr, end: VirtAddr) -> bool {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].check_allocated(start, end)
    }

    fn current_task_map(&self, start: usize, len: usize, port: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let task = &mut inner.tasks[current];

        task.memory_set.map_range(start, len, port)
    }

    fn current_task_unmap(&self, start: usize, len: usize) -> isize {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        let task = &mut inner.tasks[current];

        task.memory_set.unmap_range(start, len)
    }

}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}

///
pub fn add_syscall_count(sys_call_id: usize) {
    TASK_MANAGER.add_syscall_count(sys_call_id);
}

///
pub fn get_task_info() -> TaskInfo{
    TASK_MANAGER.get_current_task_info()
}

/// mmap
pub fn kernel_mmap(start_vpn: VirtAddr, end_vpn: VirtAddr, port: usize) -> isize {
    TASK_MANAGER.kernel_mmap(start_vpn, end_vpn, port)
}

/// mmap
pub fn kernel_unmmap(start_vpn: VirtAddr, end_vpn: VirtAddr)  {
    TASK_MANAGER.kernel_unmmap(start_vpn, end_vpn)
}

/// check all allocated
pub fn check_all_allocated(start: VirtAddr, end: VirtAddr) -> bool {
    TASK_MANAGER.check_all_allocated(start, end)
}

/// check allocated
pub fn check_allocated(start: VirtAddr, end: VirtAddr) -> bool {
    TASK_MANAGER.check_allocated(start, end)
}

/// Map current task heap
pub fn current_task_map(start: usize, len: usize, port: usize) -> isize {
    TASK_MANAGER.current_task_map(start, len, port)
}

/// Unmap current task heap
pub fn current_task_unmap(start: usize, len: usize) -> isize {
    TASK_MANAGER.current_task_unmap(start, len)
}