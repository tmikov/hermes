// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

#![no_std]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn host_add(a: i32, b: i32) -> i32;
    fn host_log(ptr: *const u8, len: usize);
}

static GREETING: &str = "hello from rust";

#[unsafe(no_mangle)]
pub extern "C" fn run(x: i32) -> i32 {
    unsafe { host_add(x, 1) }
}

#[unsafe(no_mangle)]
pub extern "C" fn greet() {
    unsafe { host_log(GREETING.as_ptr(), GREETING.len()) }
}
