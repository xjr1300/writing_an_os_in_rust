#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use blog_os::println;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    use x86_64::registers::control::Cr3;

    let (level_4_page_table, _) = Cr3::read();
    println!("Lavel 4 page table at: {:?}", level_4_page_table);

    // #[allow(unconditional_recursion)]
    // fn stack_overflow() {
    //     stack_overflow(); // for each recursion, the return address is pushed
    // }

    // // trigger a stuck overflow
    // stack_overflow();

    // // trigger a page fault
    // unsafe {
    //     *(0xdeadbeef as *mut u64) = 42;
    // }

    // new
    // let ptr = 0xdeadbeaf as *mut u32;

    // Note: The actual address might be different for you. Use the address that
    // your page fault handler reports.
    let ptr = 0x2031b2 as *mut u32;

    // read from a code page
    unsafe {
        let _x = *ptr;
    }
    println!("read worked");

    // write to a code page
    unsafe {
        *ptr = 42;
    }
    println!("write worked");

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    blog_os::hlt_loop();
}

/// テスト・モードで使用するパニック・ハンドラ
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();
}

/// テスト・モードで使用するパニック・ハンドラ
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
