#![feature(page_size)]

extern crate libc;
extern crate num;

use libc::types::common::c95::c_void;
use libc::types::os::arch::posix01;
use libc::funcs::posix88 as posix88_f;
use libc::consts::os::posix88 as posix88_c;
use libc::consts::os::extra;
use libc::funcs::c95::stdio;
use libc::types::os::arch::c95 as c95_t;

use num::traits::NumCast;

use std::ffi::CString;

use std::env;
use std::io::{self, Write};
use std::process;

fn errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(-1)
}

struct Fd {
    raw_fd: c95_t::c_int,
}

impl Fd {
    pub fn open(path: &str) -> Result<Fd, i32> {
        let c_path = CString::new(path).unwrap();
        let fd = unsafe {
            posix88_f::fcntl::open(
                c_path.as_ptr(),
                posix88_c::O_RDONLY,
                0)
        };
        if fd == -1 {
            return Err(errno());
        }

        Ok(Fd { raw_fd: fd })
    }
    pub fn raw(&self) -> c95_t::c_int {
        self.raw_fd
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        unsafe {
            posix88_f::unistd::close(self.raw_fd);
        }
    }
}

fn print_offset(offset: i64) {
    print!("{:08x}  ", offset)
}

fn print_hex(buffer: &[u8], line_width: i64) {
    for (i, c) in (0..).zip(buffer.iter()) {
        print!("{:02x} ", c);
        if i == line_width / 2 - 1 {
            print!(" ");
        }
    }
}

fn align_delimiter(line_size_current: i64, line_size_full: i64) {
    for _ in line_size_current..line_size_full {
        print!("   ");
    }

    if line_size_current < line_size_full / 2 {
        print!(" ");
    }

    print!(" |");
}

fn print_chars(buffer: &[u8]) {
    for c in buffer {
        let is_print = unsafe {
            libc::funcs::c95::ctype::isprint(
                num::traits::NumCast::from(*c).unwrap()) != 0
        };
        if is_print {
            print!("{}", *c as char);
        } else {
            print!(".")
        }
    }
}

fn print_contents(buffer: &[u8], buffer_size: i64, offset: i64) {
    if buffer_size == 0 {
        return;
    };

    let line_width_elements = 16;
    let mut remaining_buffer_size = buffer_size;
    let mut current_offset = offset;

    for line in buffer.chunks(line_width_elements as usize) {
        let line_size = if remaining_buffer_size > line_width_elements {
            line_width_elements
        } else {
            remaining_buffer_size
        };
        if line_size == 0 {
            break;
        }

        print_offset(current_offset);

        print_hex(
            line,
            line_width_elements);

        align_delimiter(line_size, line_width_elements);

        print_chars (line);

        println!("|");

        current_offset += line_size;
        remaining_buffer_size -= line_size;
    }

    print_offset(current_offset);

    println!("");
}

fn read_print_file(path: &str) -> Result<(), ()> {
    let maybe_fd = Fd::open(path);
    let fd: Fd;
    match maybe_fd {
        Ok(f) => fd = f,
        Err(_) => {
            let c_error = CString::new("Couldn't open file").unwrap();
            unsafe {
                stdio::perror(c_error.as_ptr());
            }
            return Err(());
        }
    }
    let mut file_info : posix01::stat = unsafe {
        std::mem::uninitialized()
    };
    let result = unsafe {
        posix88_f::stat_::fstat(fd.raw(), & mut file_info)
    };
    if result == -1 {
        let c_error = CString::new("Couldn't get file into").unwrap();
        unsafe {
            stdio::perror(c_error.as_ptr());
        }
        return Err(());
    }
    let mut remaining_file_size = file_info.st_size;
    let page_size : i64 = NumCast::from(std::env::page_size()).unwrap();
    let mut offset = 0;
    while remaining_file_size > 0 {
        let map_size: u64 = NumCast::from(
            if remaining_file_size > page_size {
                page_size
            } else {
                remaining_file_size
            }).unwrap();
        let address = unsafe {
            posix88_f::mman::mmap(
                0 as *mut c_void,
                map_size,
                posix88_c::PROT_READ,
                posix88_c::MAP_PRIVATE
              | extra::MAP_POPULATE,
                fd.raw(),
                offset)
        };
        if address == posix88_c::MAP_FAILED {
            let c_error = CString::new("Couldn't read file").unwrap();
            unsafe {
                stdio::perror(c_error.as_ptr());
            }
            return Err(());
        };

        let buffer : &[u8] = unsafe {
            std::slice::from_raw_parts(
                address as *const u8, NumCast::from(map_size).unwrap())
        };

        print_contents(buffer, NumCast::from(map_size).unwrap(), offset);

        let result = unsafe {
            posix88_f::mman::munmap(
                address,
                map_size)
        };
        if result == -1 {
            let c_error = CString::new("Couldn't unmap file").unwrap();
            unsafe {
                stdio::perror(c_error.as_ptr());
            }
        }

        let diff: i64 = NumCast::from(map_size).unwrap();
        remaining_file_size -= diff;
        offset += diff;
    }

    Ok(())
}

fn main() {
    let mut args = env::args();
    let mut stderr = std::io::stderr();
    if args.len() != 2 {
        writeln!(&mut stderr, "Usage: rexdump <file>").unwrap();
        process::exit(1);
    }
    let s : String = args.nth(1).unwrap();
    read_print_file(&s).unwrap();
}
