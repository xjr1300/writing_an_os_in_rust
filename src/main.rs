#![no_std] // Rust標準ライブラリにリンクしない
#![no_main] // すべてのRustのレベルのエントリポイントを無効

use core::panic::PanicInfo;

#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    loop {}
}

/// パニックが発生したときに呼び出される関数
///
/// PanicInfoは、パニックが発生したファイルと行数と、オプションでパニックメッセージを含む。
/// この関数はリターンしないため、never型を返却することにより、発散する関数としてマークした。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
