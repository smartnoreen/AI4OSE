//! Process management syscalls
use crate::task::{change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, mmap, munmap, current_user_token};
use crate::mm::translated_byte_buffer;
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let tv = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let token = current_user_token();
    let buffers = translated_byte_buffer(token, ts as *const u8, core::mem::size_of::<TimeVal>());
    let mut offset = 0;
    let tv_bytes = unsafe {
        core::slice::from_raw_parts(&tv as *const _ as *const u8, core::mem::size_of::<TimeVal>())
    };
    for buffer in buffers {
        let len = buffer.len();
        buffer.copy_from_slice(&tv_bytes[offset..offset + len]);
        offset += len;
    }
    0
}

/// Safely translate a user space address to a byte, return None if invalid
fn safe_translate_byte_read(token: usize, addr: usize) -> Option<u8> {
    use crate::mm::{PageTable, VirtAddr};
    use crate::config::TRAP_CONTEXT_BASE;
    // Addresses above TRAP_CONTEXT_BASE are kernel space, not accessible from user
    if addr >= TRAP_CONTEXT_BASE {
        return None;
    }
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(addr);
    let vpn = va.floor();
    let offset = va.page_offset();
    page_table.translate(vpn).and_then(|pte| {
        // Check if PTE is valid and readable
        if pte.is_valid() && pte.readable() {
            let ppn = pte.ppn();
            Some(ppn.get_bytes_array()[offset])
        } else {
            None
        }
    })
}

/// Safely translate and write a byte to user space address, return false if invalid
fn safe_translate_byte_write(token: usize, addr: usize, value: u8) -> bool {
    use crate::mm::{PageTable, VirtAddr};
    use crate::config::TRAP_CONTEXT_BASE;
    // Addresses above TRAP_CONTEXT_BASE are kernel space, not accessible from user
    if addr >= TRAP_CONTEXT_BASE {
        return false;
    }
    let page_table = PageTable::from_token(token);
    let va = VirtAddr::from(addr);
    let vpn = va.floor();
    let offset = va.page_offset();
    if let Some(pte) = page_table.translate(vpn) {
        // Check if PTE is valid and writable
        if pte.is_valid() && pte.writable() {
            let ppn = pte.ppn();
            ppn.get_bytes_array()[offset] = value;
            return true;
        }
    }
    false
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace request={}, id={:#x}, data={}", trace_request, id, data);
    const TRACE_READ: usize = 0;
    const TRACE_WRITE: usize = 1;

    match trace_request {
        TRACE_READ => {
            // Read a byte from user space address
            let token = current_user_token();
            if let Some(byte) = safe_translate_byte_read(token, id) {
                byte as isize
            } else {
                -1
            }
        }
        TRACE_WRITE => {
            // Write a byte to user space address
            let token = current_user_token();
            if safe_translate_byte_write(token, id, data as u8) {
                0
            } else {
                -1
            }
        }
        _ => -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap start={:#x}, len={}, port={}", start, len, port);
    mmap(start, len, port)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap start={:#x}, len={}", start, len);
    munmap(start, len)
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
