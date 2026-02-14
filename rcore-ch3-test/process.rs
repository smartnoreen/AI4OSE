//! Process management syscalls
use crate::{
    task::{exit_current_and_run_next, suspend_current_and_run_next, get_syscall_times},
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

/// trace syscall for debugging
/// - trace_request: 0=Read, 1=Write, 2=Syscall
/// - id: address for Read/Write, syscall_id for Syscall
/// - data: data to write for Write operation
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace request={} id={} data={}", trace_request, id, data);
    match trace_request {
        // TraceRequest::Read = 0
        0 => {
            // Read memory at address `id`
            // Check if address is valid (in user space)
            if id == 0 {
                return -1;
            }
            unsafe {
                let ptr = id as *const u8;
                // Try to read the byte
                // In a real OS, we should check if the address is in user space
                // For simplicity, we just try to read it
                *ptr as isize
            }
        }
        // TraceRequest::Write = 1
        1 => {
            // Write data to memory at address `id`
            if id == 0 {
                return -1;
            }
            unsafe {
                let ptr = id as *mut u8;
                *ptr = data as u8;
            }
            0
        }
        // TraceRequest::Syscall = 2
        2 => {
            // Return the syscall count for syscall_id `id`
            get_syscall_times(id) as isize
        }
        _ => -1,
    }
}
