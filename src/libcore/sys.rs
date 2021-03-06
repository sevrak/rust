// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Misc low level stuff

use cast;
use cmp::{Eq, Ord};
use gc;
use io;
use libc;
use libc::{c_void, c_char, size_t};
use repr;
use str;

pub type FreeGlue = &'self fn(*TypeDesc, *c_void);

// Corresponds to runtime type_desc type
pub struct TypeDesc {
    size: uint,
    align: uint,
    take_glue: uint,
    drop_glue: uint,
    free_glue: uint
    // Remaining fields not listed
}

/// The representation of a Rust closure
pub struct Closure {
    code: *(),
    env: *(),
}

pub mod rusti {
    #[abi = "rust-intrinsic"]
    pub extern {
        fn get_tydesc<T>() -> *();
        fn size_of<T>() -> uint;
        fn pref_align_of<T>() -> uint;
        fn min_align_of<T>() -> uint;
    }
}

pub mod rustrt {
    use libc::{c_char, size_t};

    pub extern {
        #[rust_stack]
        unsafe fn rust_upcall_fail(expr: *c_char,
                                   file: *c_char,
                                   line: size_t);
    }
}

/// Compares contents of two pointers using the default method.
/// Equivalent to `*x1 == *x2`.  Useful for hashtables.
pub fn shape_eq<T:Eq>(x1: &T, x2: &T) -> bool {
    *x1 == *x2
}

pub fn shape_lt<T:Ord>(x1: &T, x2: &T) -> bool {
    *x1 < *x2
}

pub fn shape_le<T:Ord>(x1: &T, x2: &T) -> bool {
    *x1 <= *x2
}

/**
 * Returns a pointer to a type descriptor.
 *
 * Useful for calling certain function in the Rust runtime or otherwise
 * performing dark magick.
 */
#[inline(always)]
pub fn get_type_desc<T>() -> *TypeDesc {
    unsafe { rusti::get_tydesc::<T>() as *TypeDesc }
}

/// Returns the size of a type
#[inline(always)]
pub fn size_of<T>() -> uint {
    unsafe { rusti::size_of::<T>() }
}

/**
 * Returns the size of a type, or 1 if the actual size is zero.
 *
 * Useful for building structures containing variable-length arrays.
 */
#[inline(always)]
pub fn nonzero_size_of<T>() -> uint {
    let s = size_of::<T>();
    if s == 0 { 1 } else { s }
}

/**
 * Returns the ABI-required minimum alignment of a type
 *
 * This is the alignment used for struct fields. It may be smaller
 * than the preferred alignment.
 */
#[inline(always)]
pub fn min_align_of<T>() -> uint {
    unsafe { rusti::min_align_of::<T>() }
}

/// Returns the preferred alignment of a type
#[inline(always)]
pub fn pref_align_of<T>() -> uint {
    unsafe { rusti::pref_align_of::<T>() }
}

/// Returns the refcount of a shared box (as just before calling this)
#[inline(always)]
pub fn refcount<T>(t: @T) -> uint {
    unsafe {
        let ref_ptr: *uint = cast::reinterpret_cast(&t);
        *ref_ptr - 1
    }
}

pub fn log_str<T>(t: &T) -> ~str {
    unsafe {
        do io::with_str_writer |wr| {
            repr::write_repr(wr, t)
        }
    }
}

/** Initiate task failure */
pub fn begin_unwind(msg: ~str, file: ~str, line: uint) -> ! {
    do str::as_buf(msg) |msg_buf, _msg_len| {
        do str::as_buf(file) |file_buf, _file_len| {
            unsafe {
                let msg_buf = cast::transmute(msg_buf);
                let file_buf = cast::transmute(file_buf);
                begin_unwind_(msg_buf, file_buf, line as libc::size_t)
            }
        }
    }
}

// FIXME #4427: Temporary until rt::rt_fail_ goes away
pub fn begin_unwind_(msg: *c_char, file: *c_char, line: size_t) -> ! {
    unsafe {
        gc::cleanup_stack_for_failure();
        rustrt::rust_upcall_fail(msg, file, line);
        cast::transmute(())
    }
}

pub fn fail_assert(msg: &str, file: &str, line: uint) -> ! {
    unsafe {
        let (msg, file) = (msg.to_owned(), file.to_owned());
        begin_unwind(~"assertion failed: " + msg, file, line)
    }
}

#[cfg(test)]
pub mod tests {
    use cast;
    use sys::{Closure, pref_align_of, size_of, nonzero_size_of};

    #[test]
    pub fn size_of_basic() {
        fail_unless!(size_of::<u8>() == 1u);
        fail_unless!(size_of::<u16>() == 2u);
        fail_unless!(size_of::<u32>() == 4u);
        fail_unless!(size_of::<u64>() == 8u);
    }

    #[test]
    #[cfg(target_arch = "x86")]
    #[cfg(target_arch = "arm")]
    #[cfg(target_arch = "mips")]
    pub fn size_of_32() {
        fail_unless!(size_of::<uint>() == 4u);
        fail_unless!(size_of::<*uint>() == 4u);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    pub fn size_of_64() {
        fail_unless!(size_of::<uint>() == 8u);
        fail_unless!(size_of::<*uint>() == 8u);
    }

    #[test]
    pub fn nonzero_size_of_basic() {
        type Z = [i8 * 0];
        fail_unless!(size_of::<Z>() == 0u);
        fail_unless!(nonzero_size_of::<Z>() == 1u);
        fail_unless!(nonzero_size_of::<uint>() == size_of::<uint>());
    }

    #[test]
    pub fn align_of_basic() {
        fail_unless!(pref_align_of::<u8>() == 1u);
        fail_unless!(pref_align_of::<u16>() == 2u);
        fail_unless!(pref_align_of::<u32>() == 4u);
    }

    #[test]
    #[cfg(target_arch = "x86")]
    #[cfg(target_arch = "arm")]
    #[cfg(target_arch = "mips")]
    pub fn align_of_32() {
        fail_unless!(pref_align_of::<uint>() == 4u);
        fail_unless!(pref_align_of::<*uint>() == 4u);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    pub fn align_of_64() {
        fail_unless!(pref_align_of::<uint>() == 8u);
        fail_unless!(pref_align_of::<*uint>() == 8u);
    }

    #[test]
    pub fn synthesize_closure() {
        unsafe {
            let x = 10;
            let f: &fn(int) -> int = |y| x + y;

            fail_unless!(f(20) == 30);

            let original_closure: Closure = cast::transmute(f);

            let actual_function_pointer = original_closure.code;
            let environment = original_closure.env;

            let new_closure = Closure {
                code: actual_function_pointer,
                env: environment
            };

            let new_f: &fn(int) -> int = cast::transmute(new_closure);
            fail_unless!(new_f(20) == 30);
        }
    }
}

// Local Variables:
// mode: rust;
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
