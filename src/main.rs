#![no_std] // Rust標準ライブラリにリンクしない
#![no_main] // すべてのRustのレベルのエントリポイントを無効

use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello World";

/// パニックが発生したときに呼び出される関数
///
/// PanicInfoは、パニックが発生したファイルと行数と、オプションでパニックメッセージを含む。
/// この関数はリターンしないため、never型を返却することにより、発散する関数としてマークした。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}
