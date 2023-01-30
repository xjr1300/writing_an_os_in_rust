# テスト

この投稿は、`no_std`実行形式における、単体及び統合テストを探求します。
我々のカーネル内のテスト関数を実行するために、Rustがサポートするカスタム・テスト・フレームワークを使用する予定です。
QEMEの外部にその結果を出力するために、QEMEのさまざまな機能と`bootimage`ツールを使用する予定です。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/testing/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-04`](https://github.com/phil-opp/blog_os/tree/post-04)ブランチで見つけることができます。

## 必須要件

このポストは（現在非推奨になった）[*Unit Testing*](https://os.phil-opp.com/unit-testing/)と[*Integration Tests*](https://os.phil-opp.com/integration-tests/)の投稿を置き換えたもためす。
それは、2019-04-27以降の[*A Minimal Reust Kernel*](https://os.phil-opp.com/minimal-rust-kernel/)の投稿に従っていることを想定しています。
主に、それは、[デフォルト・ターゲットを設定して](https://os.phil-opp.com/minimal-rust-kernel/#set-a-default-target)、そして[実行可能なランナーを定義する](https://os.phil-opp.com/minimal-rust-kernel/#using-cargo-run)`.cargo/config.toml`ファイルがあることを要求しています。

## Rustにおけるテスト

Rustは、何も準備することなく単体テストを実行することができる[ビルトインされたテスト・フレームワーク](https://doc.rust-lang.org/book/ch11-00-testing.html)があります。
アサーションを通じていくつかの結果を確認する関数を作成して、関数のヘッダに`#[test]`属性を追加するだけです。
そして、`cargo test`はクレートのすべてのテスト関数を検索して、実行します。

不運にも、我々のカーネルのような`no_std`アプリケーションにとって、それは少し複雑です。

問題は、Rustのテスト・フレームワークは、暗黙的にビルトインされた[`test`](https://doc.rust-lang.org/test/index.html)ライブラリを使用することで、それは標準ライブラリに依存しています。
これは、我々の`#[no_std]`カーネルのために、デフォルトのテスト・フレームワークを使用できないことを意味しています。

我々のプロジェクトで`cargo test`の実行を試みると、これを見ることができます。

```
> cargo test
   Compiling blog_os v0.1.0 (/…/blog_os)
error[E0463]: can't find crate for `test`
```

`test`クレートが標準ライブラリに依存しているため、それを我々のベア・メタル・ターゲットのために利用できません。
`test`クレートを`#[no_std]`コンテキストに移植することは[可能](https://github.com/japaric/utest)ですが、それはとても不安定で、`panic`マクロを再定義するような、いくつかをうまくやり遂げる必要があります。

### カスタム・テスト・フレームワーク

幸運にも、Rustは不安定な[`custom_test_frameworks`](https://doc.rust-lang.org/unstable-book/language-features/custom-test-frameworks.html)機能を通じて、デフォルトのテスト・フレームワークの置き換えをサポートしています。
この機能は外部ライブラリを必要とせず、従って`[#no_std]`環境においても動作します。
それは`#[test_case]`属性で注釈されたすべての関数を集め、テストのリストを引数としてユーザーが指定したランナー関数を呼び出すことによって機能します。
従って、実装はテスト処理を最大限に制御できます。

デフォルトのテスト・フレームワークと比較した欠点は、[`should_panic`テスト](https://doc.rust-lang.org/book/ch11-01-writing-tests.html#checking-for-panics-with-should_panic)のような、多くの高度な機能がないことです。
そのような高度な機能のデフォルトの実装が、おそらく動作しない特別な実行環境があるため、我々にとってこれは理想的です。
例えば、我々のカーネル用のカスタム・テスト・フレームワークを実装するために、`main.rs`に次を追加します。

```rust
// in src/main.rs

#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} test", tests.len());
    for test in tests {
        test();
    }
}
```

我々のランナーは単に短いデバッグ・メッセージを出力して、リスト内の各テスト関数を呼び出します。
引数の型`&[&dyn Fn()]`は、[*Fn()*](https://doc.rust-lang.org/std/ops/trait.Fn.html)[*トレイトオブジェクト*](https://doc.rust-lang.org/1.30.0/book/first-edition/trait-objects.html)参照の[*スライス*](https://doc.rust-lang.org/std/primitive.slice.html)です。
非テスト実行用にその関数は使い物にならないため、テスト用のみの場合に含める`#[cfg(test)]`属性を使用しています。

現在、`cargo test`を実行したとき、成功することがわかります（成功しない場合は、以下の注意を参照してください）。
しかしながら、我々の`test_runner`からのメッセージに代わって、未だに"Hello World!"が表示されています。
その理由は、我々の`_start`関数が、未だにエントリ・ポイントとして使用されているからです。
カスタム・テスト・フレームワーク機能は、`test_runner`を呼び出す`main`関数を生成しますが、`#[no_main]`属性を使用して、独自のエントリ・ポイントを提供しているため、この関数は無視されます。

---

いくつかのケースにおいて、`cargo test`で"duplicate lang item"エラーを招くバグが、現在cargoに存在します。
それは、`Cargo.toml`のプロファイルに`panic = "abort"`を設定した場合に発生します。
それを削除して見てください。その後、`cargo test`が機能するはずです。
詳細については、[cargoの問題](https://github.com/rust-lang/cargo/issues/7359)を参照してください。

---

これを修正するために、最初に`reexport_test_harness_main`属性を通じて生成する関数を`main`とは違う名前に変更する必要があります。
それで、`_start`関数から名前を付け直された関数を呼び出すことができます。

```rust
// in src/main.rs

#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}
```

テスト・フレームワークのエントリ・ポイントの名前を`test_main`に設定して、`_start`エントリ・ポイントからそれを呼び出します。
`test_main`関数は通常の実行で生成されないため、テスト・コンテキスト内でのみ`test_main`への呼び出しを追加するために、[条件付きコンパイル](https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html)を使用します。

現在、`cargo test`を実行したとき、スクリーンに我々の`test_runner`からの"Running 0 tests"メッセージが見えます。
現在、最初のテスト関数を作成する準備ができました。

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    print!("trivial assertion...");
    assert_eq!(1, 1);
    println!("[ok]");
}
```

現在、`cargo test`を実行したとき、次の出力が見えます。

![trivial assertion](https://os.phil-opp.com/testing/qemu-test-runner-output.png)

現在、`test_runner`関数に渡された`test`スライスは、`trivial_assertion`関数への参照を含んでいます。
スクリーンからの`trivial assertion... [ok]`の出力から、テストが呼び出されて、それが成功したことを確認できます。

テストを実行した後、`test_runner`は`test_main`関数に戻り、順番に`_start`エントリ・ポイント関数に戻ります。
`最後の`_start`では、エントリ・ポイント関数は戻ることを許されていないため、終わりのないループに入ります。
すべてのテストが実行された後、`cargo test`を終了することが望ましいため、これは問題です。

## QEMUの終了

たった今、`_start`関数の最後に無限ループがあるため、`cargo test`の各実行で手動でQEMUを閉じる必要があります。
ユーザーと対話することなくスクリプト内で`cargo test`を実行したいと考えているので、これは不運です。
これに対する良い解決方法は、適切な方法で我々のOSを停止する実装をすることでしょう。
不運にも、[APM](https://wiki.osdev.org/APM)または[ACPI](https://wiki.osdev.org/ACPI)電源管理標準のどちらかをサポートする実装を要求するため、これは比較的難しいです。

幸運にも、脱出口があります。
QEMEは特別な`isa-debug-exit`デバイスをサポートしており、それはゲスト・システムからQEMEを終了するための簡単な方法を提供しています。
それを有効にするためにQEMEに`-device`引数を渡す必要があります。
`Cargo.toml`の`package.meadata.bootimage.test-args`設定キーを追加することでこれができます。

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]
```

`bootimage runner`は、すべてのテストの実行で、`test-args`をドフォルトのQEMEコマンドに追加します。
普通の`cargo run`は引数を無視します。

デバイス名（`isa-debug-exit`）と一緒に、我々のカーネルからデバイスに届くように、*I/Oポート*を指定した`iobase`と`iosize`の2つのパラメータを渡します。

### I/Oポート

x86にはCPUと物理ハードウェアが会話する**メモリ・マップドI/O**と**ポート・マップドI/O**という、2つの異なる方法があります。
メモリ・アドレス`0xb8000`を通じて[VGAテキスト・バッファ](https://os.phil-opp.com/vga-text-mode/)にアクセスするために、メモリ・マップドI/Oを既に使用しました。
このアドレスは、RAMへマッピングされていませんが、VGAデバイスのメモリにマッピングされています。

対称的に、ポート・マップドI/Oは会話するために分離したI/Oバスを使用します。
互いに接続した周辺機器は一つまたは複数のポート番号を持っています。
そのようなI/Oポートと会話するために、`in`と`out`と呼ばれる特別なCPU命令があり、それはポート番号とデータ・バイトを受け取ります（`u16`または`u32`を送信できるこれらのコマンドにはいろいろな種類があります）。

`isa-debug-exit`デバイスはポート・マップドI/Oを使用します。
`iobase`パラメータはどのポートのアドレスにデバイスが接続するべきかを指定して（`0xf4`はx86のIOバスで[一般に使用されない](https://wiki.osdev.org/I/O_Ports#The_list)ポートです）、`iosize`はポート・サイズを指定します（`0x04`は4バイトを意味します）。

### 脱出デバイスの使用

`isa-debug-exit`デバイスの機能性はとても単純です。
`値`が`iobase`で指定されたI/Oポートに書き込まれたとき、それは[終了ステータス](https://en.wikipedia.org/wiki/Exit_status)`(value << 1) | 1`でQEMEを終了します。
よって、`0`をポートに書き込んだとき、QEMEは終了ステータス`(0 << 1) | 1 = 1)`でQEMUを終了して、`1`を書き込んだとき、終了ステータス`(1 << 1 | 1 = 3)`で終了します。

`in`と`out`のアセンブリ命令を手動で呼び出す代わりに、[`x86_64`](https://docs.rs/x86_64/0.14.2/x86_64/)クレートによって提供される抽象化を使用します。
そのクレートの依存関係を追加するために、`Cargo.toml`の`dependencies`にそれを追加します。

```toml
// in Cargo.toml

[dependencies]
x86_64 = "0.14.2"
```

現在、`exit_qeme`関数を作成するために、そのクレートによって提供される[`Port`](https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html)型を使用できます。

```rust
// in src/main.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

その関数は`isa-debug-exit`デバイスの`iobase`である`0xf4`に新しいポートを作成します。
そしてそれは、そのポートに渡された終了コードを書き込みます。
`isa-debug-exit`デバイスの`iosize`として4バイトを指定しているので、`u32`を使用しています。
両方の操作は、I/Oポートへの書き込みは一般的に任意の振る舞いをするので、不安定です。

終了ステータスを指定するために、`QemuExitCode`列挙型を作成します。
その考えは、もしすべてのテストが成功した場合は成功終了コードで終了して、それ以外の場合は失敗終了コードで終了することです。
その列挙型は`u32`によってそれぞれのバリアントを表現するために、`#[repr(u32)]`としてマークされています。
成功したとき終了コード`0x10`を、失敗したときは終了コード`0x11`を使用します。
QEMUのデフォルトの終了コードと衝突しない限り、実際の終了コードはあまり重要ではありません。
例えば、成功のために、終了コード`0`を使用することは、変換された後で`(0 << 1) | 1 = 1`になり、それはQEMEが実行に失敗したときのデフォルト終了コードであるため、良い考えではありません。
そのため、QEMEのエラーとテスト実行の成功を区別できません。

現在、すべてのテストが実行された後で、QEMEを終了する`test_runner`を更新できます。

```rust
// in src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    // QEME終了
    exit_qemu(QemuExitCode::Success);
}
```

現在、`cargo test`を実行したとき、テストが実行された後で、QEMEがすぐに閉じることを確認できます。
その問題は、`cargo test`が、渡した我々の`Success`終了コードを通じて、テストに失敗したと解釈することです。

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.03s
     Running target/x86_64-blog_os/debug/deps/blog_os-5804fc7d2dd4c9be
Building bootloader
   Compiling bootloader v0.5.3 (/home/philipp/Documents/bootloader)
    Finished release [optimized + debuginfo] target(s) in 1.07s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-5804fc7d2dd4c9be.bin -device isa-debug-exit,iobase=0xf4,
    iosize=0x04`
error: test failed, to rerun pass '--bin blog_os'
```

その問題は、`cargo test`は`0`以外のすべてのエラーコードを終了とみなすことです。

### 成功終了コード

これを回避するために、`bootimage`は指定した終了コードを終了コード`0`にマップする、`test-success-exit-code`設定キーを提供しています。

```toml
# in Cargo.toml

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,issize=0.x04"]
test-success-exit-code = 33     # (0x10 << 1) | 1
```

この設定で、`bootimage`は我々の成功終了コードを終了コード0にマップするので、`cargo test`は正確に成功したケースを認識して、失敗したとしてテストをカウントしません。

現在、我々のテスト・ランナーは自動的にQEMUを閉じて、正しくテストの結果を報告するようになりました。
いまだ、とても短時間QEMUのウィンドウが見えますが、結果を読むことに十分ではありません。
もし、代わりにテストの結果をコンソールに出力できれば、それらをQEMUが終了した後でも確認できるため、素晴らしいでしょう。

## コンソールへの出力

コンソールでテストの出力を確認するために、なんらかの方法で、我々のカーネルからホスト・システムにデータを送信する必要があります。
例えば、TCPネットワーク・インターフェースでデータを送信することによるなど、これを実現するためにはいろいろな方法があります。
しかしながら、ネットワーキング・スタックを準備することは、とても複雑なタスクなので、代わりにより単純な解決方法を選択します。

### シリアル・ポート

データを送信する簡単な方法は、[シリアル・ポート](https://en.wikipedia.org/wiki/Serial_port)を使用することで、古い標準的なインターフェースで、現在のコンピューターでは、もはや見つけることができません。
それはプログラムすることが簡単で、QEMUはバイトをシリアルで送信して、ホストの標準出力かファイルにリダイレクトできます。

シリアル・インターフェースを実装したチップは[UARTs](https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter)と呼ばれています。
x86には[多くのUARTモデル](https://en.wikipedia.org/wiki/Universal_asynchronous_receiver-transmitter#UART_models)がありますが、幸運にも、単なる違いは、我々が使用しない何らか高度な機能です。
現在の一般的なUARTsはすべて[16550 UART](https://en.wikipedia.org/wiki/16550_UART)と互換性があるので、テスト・フレームワークにそのモデルを使用する予定です。

UARTを初期化して、シリアル・ポートでデータを送信するために、[`uart_16550`]クレートを使用する予定です。
依存関係としてそれを追加するために、`Cargo.toml`と`main.rs`を更新します。

```toml
# in Cargo.toml

[dependencies]
uart_16550 = "0.2.0"
```

`uart_16550`クレートは、UARTレジスターを表現する`SerialPort`構造体を含みますが、そのインスタンを我々自身で構築する必要があります。
そのため、以下の内容で新しい`serial`モジュールを作成します。

```rust
// in src/main.rs

mod serial;
```

```rust
// in src/serial.rs

use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3f8) };
        serial_port.init();

        Mutex::new(serial_port)
    };
}
```

[VGAテキスト・バッファ]()と同様に、`lazy_static`と`静的な`ライターインスタンスを作成するために、スピンロックを使用します。
`lazy_static`を使用することにより、それが最初に使用されたとき、`init`メソッドが正確に１回のみ呼び出されることを保証します。

`isa-debug-exit`デバイスのように、UARTはポートI/Oを使用してプログラムされます。
UARTはより複雑なため、それは複数のI/Oポートを使用して、異なるデバイス・レジスタをプログラミングします。
アンセーフな`SerialPort::new`関数は、引数としてUARTの最初のI/Oポートのアドレスを予期しており、それからすべての必要なポートのアドレスを計算できます。
最初のシリアル・インターフェースの標準的なポート番号であるポート・アドレス`0x3F8`を渡しています。

シリアル・ポートを簡単に利用しやすくするために、`serial_print!`と`serial_println!`マクロを追加します。

```rust
// in src/serial.rs


#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

/// シリアル・インターフェースを通じてホストに出力
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    }
}

// 改行を追加して、シリアル・インターフェースを通じてホストに出力
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt,"\n"), $($arg)*));
}
```

実装は、我々の`print`と`println`マクロの実装と、とても良く似ています。
`SerialPort`型は既に[`fmt::Write`](https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html)トレイトを実装しているため、我々独自の実装に提供する必要はありません。

現在、テスト・コード内にVGAテキスト・バッファの代わりにシリアル・インターフェースに出力することができます。

```rust
// in src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
    // QEME終了
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion...");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

`serial_println`マクロは、`#[macro_export]`属性を使用しているので、ルート名前空間の直下にあります。よって、`use crate::serial::serial_println`を通じてそれをインポートすることは機能しません。

### QEMUの引数

QEMUからのシリアル出力を確認するために、標準出力への出力をリダイレクトするために`-serial`引数を使用する必要があります。

```toml
// in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device",
    "isa-debug-exit,iobase=0xf4,issize=0.x04",
    "-serial",
    "stdio"
]
test-success-exit-code = 33 # (0x10 << 1) | 1
```

現時点で`cargo test`を実行したとき、直接コンソール内でテストの出力を確認できます。

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [ok]
```

しかしながら、テストが失敗したとき、我々のパニック・ハンドラが未だ`println`を使用しているため、未だQEMU内で出力を確認します。
これを模倣するために、`trivial_assertion`テストを`assert_eq(0, 1);`に変更できます。

![still message in QEMU](https://os.phil-opp.com/testing/qemu-failed-test.png)

他のテストの出力がシリアル・ポートに出力されるのに対して、パニック・メッセージは未だVGAバッファに出力されることを確認できます。
パニック・メッセージはとても役に立つので、それもコンソールで確認することは便利です。

### パニック時にエラー・メッセージを出力する

パニックが発生したときにエラー・メッセージを伴ってQEMUを終了するために、テスト・モードにおいて、異なるパニック・ハンドラを使用するために[条件付きコンパイル](https://doc.rust-lang.org/1.30.0/book/first-edition/conditional-compilation.html)を使用できます。

```rust
// in src/main.rs

///
/// PanicInfoは、パニックが発生したファイルと行数と、オプションでパニックメッセージを含む。
/// この関数はリターンしないため、never型を返却することにより、発散する関数としてマークした。
#[cfg(not(test))] // 新しい属性
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}

/// テスト・モードで使用するパニック・ハンドラ
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}", info);
    exit_qemu(QemuExitCode::Failed);

    loop {}
}
```

テスト・パニック・ハンドラのために、`println`の代わりに`serial_println`を使用して、次に失敗終了コードでQEMUを終了します。
コンパイラは、`isa-debug-exit`デバイスがプログラムの終了を引き起こしたことを知らないので、未だ`exit_qemu`呼び出しの後の無限ループが必要になることに、注意してください。

現時点で、QEMUはテストの失敗により終了して、コンソールに役に立つエラー・メッセージを出力します。

```
> cargo test
    Finished dev [unoptimized + debuginfo] target(s) in 0.02s
     Running target/x86_64-blog_os/debug/deps/blog_os-7b7c37b4ad62551a
Building bootloader
    Finished release [optimized + debuginfo] target(s) in 0.02s
Running: `qemu-system-x86_64 -drive format=raw,file=/…/target/x86_64-blog_os/debug/
    deps/bootimage-blog_os-7b7c37b4ad62551a.bin -device
    isa-debug-exit,iobase=0xf4,iosize=0x04 -serial stdio`
Running 1 tests
trivial assertion... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `0`,
 right: `1`', src/main.rs:65:5
```

現時点で、コンソールですべてのテストの出力を確認できるので、短時間だけポップ・アップするQEMUのウィンドウは、もはや必要ありません。
したがって、完全にそれを隠すことができます。

### QEMUを隠す

`isa-debug-exit`デバイスとシリアル・ポートを使用して、テストの結果を完全に出力できるようになったので、もうQEMUのウィンドウは必要でありません。
QEMUに`-display none`引数を渡すことにより、それを隠すことができます。

```toml
// in Cargo.toml

[package.metadata.bootimage]
test-args = [
    "-device",
    "isa-debug-exit,iobase=0xf4,iosize=0x04",
    "-serial",
    "stdio",
    "-display",
    "none",
]
test-success-exit-code = 33 # (0x10 << 1) | 1
```

現時点で、QEMUは完全にバックgラウンドで実行され、もはやウィンドウが開くことはありません。
これは煩わしさを軽減するだけでなく、CIサービスや[SSH](https://en.wikipedia.org/wiki/Secure_Shell)接続のようなグラフィカル・ユーザー・インターフェースを持たないような環境でテスト・フレームワークを実行できるようにします。

### タイムアウト

`cargo test`はテスト・ランナーが終了するまで待っているので、決して戻らないテストは永遠にテスト・ランナーをブロックすることができます。
それは不運ですが、通常無限ループを避けることが簡単なので、実際には大きな問題ではありません。
しかしながら、我々の場合、無限ループはさまざまな状況で発生する可能性があります。

* ブートローダーが我々のカーネルのロードに失敗すると、永遠にシステムの再起動を引き起こします。
* BIOS/UEFIファームウェアがブートローダーのロードに失敗すると、同じく永遠の再起動を引き起こします。
* 例えば、QEMU終了デバイスが適切に動作しない場合、CPUは我々の何らかの関数の末尾の`loop {}`文に入ります。
* 例えば、CPU例外がキャッチされないとき、ハードウェアがシステムのリセットを起こします（今後の投稿で説明します）。

無限ループが多くの状況で発生するため、`bootimage`ツールはデフォルトでそれぞれのテストの実行のタイムアウトを5分間に設定します。
もし、テストがこの時間内に終了しなかった場合、それは失敗としてマークされて、コンソールに"Timed Out"エラーが出力されます。
この機能は、無限ループで立ち往生したテストが、永遠に`cargo test`をブロックしないことを保証します。

`trivial_assertion`テスト内に`loop {}`文を追加することで、それを試すことができます。
`cargo test`を実行したとき、テストが５分後にタイム・アウトとしてマークされたことを確認できます。
タイムアウト時間は、Cargo.toml内の`test-timeout`キーで[設定可能](https://github.com/rust-osdev/bootimage#configuration)です。

```toml
test-timeout = 300 # in seconds
```

もし、`trivial_assertion`テストのタイムアウトのために5分間待てないのであれば、上記の値を一時的に減らすことができます。

### 自動出力の挿入

我々の`trivial_assertion`テストは、現在`serial_print!` / `serial_println!`を使用して、それ自身の状態の情報を出力する必要があります。

```rust
#[test_case]
fn trivial_assertion() {
    serial_print!("trivial assertion... ");
    assert_eq!(1, 1);
    serial_println!("[ok]");
}
```

記述するすべてのテストで手動でこれらの出力分を追加することは面倒なので、自動てこれらのメッセージを出力するように`test_runner`を更新しましょう。
それをするために、新しい`Testable`トレイトを作成する必要があります。

```rust
// in src/main.rs

pub trait Testable {
    fn run(&self) -> ();
}
```

現在のトリックは、[`Fn()`トレイト]()を実装するすべての型`T`に、このトレイトを実装します。

```rust
// in src/main.rs

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}
```

[`any::type_name`]関数を使用して、最初に関数の名前を出力する`run`関数を実装します。
この関数は、コンパイラ内に直接実装されて、すべての型の文字列の説明を返却します。
関数の場合、その型はその名前なので、このケースにおいて正確に望んでいたものです。
`\t`文字は[タブ文字]()で、それは何らかの調整を`[ok]`メッセージに追加します。

関数の名前を出力した後、`self()`を通じてテスト関数を呼び出します。
`self`が`Fn()`トレイトを実装することを要求しているの、これは機能します。
テスト関数から戻った後、その関数がパニックしなかったことを示すために`[ok]`を出力します。

最後のステップは、新しい`Testable`トレイトを使用した`test_runner`を更新することです。

```rust
// in src/main.rs

#[cfg(test)]
fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // new
    }
    // QEME終了
    exit_qemu(QemuExitCode::Success);
}
```

その単なる2つの変更は、`tests`引数を`&[&dyn Fn()]`から`&[&dyn Testable]`に、それと`test()`の代わりに`test.run()`を呼び出すようになったことです。

現時点で、`trivial_assertion`テストは自動で出力されるため、出力文を削除できます。

```rust
// in src/main.rs

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
```

現時点で、`cargo test`の出力はこのように見えます。

```
Running 1 tests
blog_os::trivial_assertion...	[ok]
```

現時点で、関数名には関数までのフル・パスが含まれているので、異なるモジュールのテスト関数が同じ名前を持つとき便利です。
一方で、出力は前と見た目が似ていますが、もはや手動でテストに出力分を追加する必要はありません。

## VGAバッファのテスト

現時点で、機能するテスト・フレームワークを手に入れたので、VGAバッファの実装のために幾つかテストを作成できます。
最初に、パニックなしで`println`が起動するかを検証するとても簡単なテストを作成します。

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}
```

そのテストは何かをVGAバッファに出力します。
それがパニックなしで終了した場、それは`println`呼び出しががパニックになかったことを意味します。

多くのラインが出力されて、スクリーンの外にシフトされてもパニックが発生しないことを保証するために、他のテストを作成できます。

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}
```

スクリーンに本当に出力された行が表示されるか検証するテスト関数も作成できます。

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
```

その関数は、テスト文字列を定義して、`println`を使用してそれを出力します。そして、VGAテキスト・バッファを表現する静的`ライタ`のスクリーン文字を順に繰り返します。
`println`はスクリーンの最終行を出力して、その後すぐに改行が現れるので、その文字列は`BUFFER_HEIGHT - 2`行に現れるはずです。

[`enumerate`](https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate)を使用することで、`c`に対応するスクリーン文字をロードするために使用する変数`i`で繰り返し回数をカウントしています。
スクリーン文字の`ascii_character`と`c`を比較することにより、文字列の各文字が本当にVGAテキスト・バッファに現れることを保証しています。

想像できるように、より多くのテスト関数を作成できます。
例えば、とても長い行を出力して、それらが正しく折り返しされることを確認する関数、または改行、出力できない文字、ユニコード文字でない文字が正しく扱われたかを確認する関数です。

しかしながら、この投稿の残りでは、異なるコンポーネントと一緒に相互作用することを確認する*統合テスト*を作成する方法を説明する予定です。

## 統合テスト

Rustにおける[統合テスト](https://doc.rust-lang.org/book/ch11-03-test-organization.html#integration-tests)の慣習は、プロジェクト・ルート内に`tests`ディレクトリ内にそれらを入れることです（例えば、`src`ディレクトリと同じ階層）。
デフォルトのテスト・フレームワークとカスタムなテスト・フレームワーク双方は、そのディレクトリ内のすべてのテストを自動的に取り出し、実行します。

すべての統合テストはそれら自身の実行形式で、我々の`main.rs`から完全に分離しています。
これは、それぞれのテストが、それ独自のエントリ・ポイント関数を定義する必要があることを意味します。
詳細にそれがどのように機能するかを確認するために、`basic_boot`と名前をつけた統合テストの例を作成しましょう。

```rust
// in tests/basic_boot.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

#[no_mangle] // この関数の名前をわからないようにしない
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

fn test_runner(tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
```

統合テストは分離された実行形式であるため、再度すべてのクレート属性（`no_std`、`no_main`、`test_runner`など）を提供する必要があります。
また、テストのエントリ・ポイント関数`test_main`が呼び出す新しいエントリ・ポイント関数`_start`も作成する必要があります。
統合テストの実行形式は非テスト・モードで決してビルドされないため、`cfg(test)`属性を付ける必要はありません。

現時点では、`test_runner`のために代替物として常にパニックする[`uninplemented`](https://doc.rust-lang.org/core/macro.unimplemented.html)マクロを使用して、`panic`ハンドラでは単に`loop`を置いています。
理想的には、`serial_println`マクロと`exit_qemu`関数を使用して、`main.rs`で実施したことと同じように、これらの関数を正確に実装したいと考えています。
問題は、テストが我々の`main.rs`実行形式と完全に分離してビルドされるため、これらの関数にアクセスできないことです。

もし、この段階で`cargo test`を実行したとき、パニック・ハンドラが無限にループするため、無限ループに突入します。
QEMUを終了するために`Ctrl + C`キーボード・ショートカットを使用する必要があります。

### ライブラリの作成

我々の統合テストで必要な関数を利用できるようにするために、`main.rs`から切り離したライブラリを作成する必要があり、それは他のクレートと統合テストの実行形式に含めることができます。
これをするために、新しい`src/lib.rs`ファイルを作成します。

```rust
// src/lib.rs

#![no_std]
```

`main.rs`のように、`lib.rs`はcargoによって自動的に認識される特別なファイルです。
そのライブラリは別のコンパイル単位なので、再度`#![no_std]`属性を指定する必要がある。

我々のライブラリが`cargo test`と一緒に機能するようにするために、テスト関数と属性を`main.rs`から`lib.rs`に再度移動する必要がある。

```rust
// in src/lib.rs

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicINfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);

    loop {}
}

/// `cargo test`のエントリ・ポイント
#[cfg(test)]
#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    test_main();

    loop {}
}

/// パニックが発生したときに呼び出される関数
///
/// PanicInfoは、パニックが発生したファイルと行数と、オプションでパニックメッセージを含む。
/// この関数はリターンしないため、never型を返却することにより、発散する関数としてマークした。
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_Handler(info);
}
```

`test_runner`を実行形式と統合テストで利用できるようにするために、公開して、それに`cfg(test)`属性を適用しないようにします。
また、実行可能ファイルでも使用できるうように、パニック・ハンドラの実装をパブリックなtest_panic_handler関数に分離します。

我々の`lib.rs`が`main.rs`と独立してテストされるように、テスト・モードでライブラリがコンパイルされたとき、`_start`エントリ・ポイントとパニック・ハンドラを追加する必要があります。
この場合、[`cfg_attr`]()クレート属性を使用することで、条件付きで`no_main`属性を有効にします。

`QemuExitCode`列挙型と`exit_qemu`関数も移動して、それらをパブリックにします。

```rust

#[derivd(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

現段階で、実行形式と統合テストはライブラリからこれらの異関数をインポートでき、それら独自の実装を定義する必要はありません。
`println`と`serial_println`を利用できるようにするために、モジュールの実装も移動します。

```rust
// in src/lib.rs

pub mod serial;
pub mod vga_buffer;
```

我々のライブラリの外側からそれらを使用できるようにするために、モジュールをパブリックにします。
これは、`println`と`serial_println`がそのモジュールの`_print`関数を使用するため、我々の`println`と`serial_println`マクロを使用できるようにすることが要求されます。

現段階で、ライブラリを使用するために我々の`main.rs`を更新できます。

```rust
// in src/main.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(blog_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use blog_os::println;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    loop {}
}

/// テスト・モードで使用するパニック・ハンドラ
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}

/// テスト・モードで使用するパニック・ハンドラ
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

そのライブラリは普通の外部クレートのように使用できます。
それは、我々のクレートと同様に、`blog_os`と呼ばれます。
上のコードは、`test_runner`属性の中の`blog_os::test_runner`関数と、我々の`cfg(test)`パニック・ハンドラ内の`blog_os::test_panic_handler`関数を使用します。
それは、我々の`_start`と`panic`関数を利用できるようにするために`println`マクロもインポートします。

この時点で、`cargo run`と`cargo test`は再度起動するはずで、`cargo test`は未だ無限ループします（`Ctrl + C`で終了できます）。
我々の統合テスト内の必須とするライブラリを使用して、これを修正しましょう。

### 統合テストの完成

我々の`src/main.rs`と同様に、我々の`tests/basic_bot.rs`実行形式は我々の新しいライブラリから型をインポートできます。
これは、我々のテストを完成させるために、不足しているコンポーネントをインポートできるようにします。

```rust
// in tests/basic_boot.rs

#![test_runner(blog_os::test_runner)]

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

テスト・ランナーを再実装する代わりに、`#![test_runner(crate::test_runner)]`属性を`#![test_runner(blog_os::test_runner)]`に変更することにより、我々のライブラリから`test_runner`関数を使用します。
`basic_boot.rs`に`test_runner`スタブ関数は必要ないので、それを削除することができます。
我々のパニック・ハンドラのために、我々の`main.rs`内でしたように、`blog_os::test_panic_handler`を呼び出します。

現時点で、`cargo test`は再び普通に終了します。
それを実行したとき、それが我々の`lib.rs`、`main.rs`そして`basic_boot.rs`を、次々と別々にビルドしてテストすることを確認できます。
`main.rs`と`basic_boot`統合テストのために、これらのファイルが`#[tst_case]`で注釈された関数を持っていないので、それは"Running 0 tests"と報告します。

現在、`basic_root.rs`にテストを追加することができます。
例えば、VGAバッファのテストで実施したことと同様に、パニックなして`println`が機能するかテストすることができます。

```rust
// in tests/basic_boot.rs

use blog_os::println;

#[test_case]
fn test_println() {
    println!("test_println output");
}
```

現時点で`cargo test`を実行したとき、それがテスト関数を見つけて実行することを確認できます。

テストがほとんどVGAバッファの1つ同じのため、現時点であまり役に立たないように見えるかもしれません。
しかし、将来、我々の`main.rs`と`lib.rs`の`_start`関数が大きくなり、`test_main`関数を実行する前にさまざまな初期化ルーチンを呼び出すようになった場合、その2つのテストはとても異なる環境で実行されます。

`_start`内で初期化ルーチンを呼び出さない`basic_boot`環境内の`println`のテストによって、ブートしたあとで`println`が正しく機能することを保証できます。
例えば、パニック・メッセージを出力するためにそれに依存しているため、これはとても重要です。

### 将来のテスト

統合テストの力は、それらが完全に分離した実行形式として扱われることです。
これは、それらに環境に対して完全に制御できるようになり、CPUやハードウェア機器と正しく相互作用するコードをテストすることを可能にします。

我々の`basic_root`テストは統合テストのとても単純な例です。
将来、我々のカーネルがより特徴的になり、さまざまな方法でハードウェアと相互作用するようになります。
統合テストを追加することにより、にこれらの相互作用が予期したように機能すること（そして機能し続けること）を保証できます。

* **CPU例外:** コードが不正な操作を実行したとき（例えば、0で割るなど）、CPUは例外を送出します。
  カーネルはそのような例外のハンドラ関数を登録できます。
  統合テストはCPU例外が発生したときに正しく例外ハンドラが呼ばれたか、また解決可能な例外の後で実行を正常に続けれるか検証できます。
* **ページ・テーブル:** ページ・テーブルはどのメモリ領域が有効でアクセス可能化を定義します。
  ページ・テーブルを編集することで、例えばプログラムが実行されたときなど、それは新しいメモリ領域を確保することができます。
  統合テストは`_start`関数内でページ・テーブルを編集して、`#[test_case]`関数内でその編集が望んでいた効果を得られたか検証することができます。
* **ユーザー空間プログラム:** ユーザー空間プログラムはシステム資源へのアクセスが制限されたプログラムです。
  例えば、それらはカーネル・データ構造体や、他のプログラムのメモリにアクセスすることができません。
  統合テストは、禁止された操作を実行するユーザー空間プログラムを実行して、カーネルがすべてそれらを防止することを検証することができます。

想像するように、より多くのテストが可能です。
そのようなテストを追加するために、我々が新しい機能を追加したときや、我々のコードをリファクタしたとき、それらを不注意で破壊しないことを保証できます。
これは、我々のカーネルが大きくより複雑になったとき、特に重要です。

### パニックするべきテスト

標準ライブラリのテスト・フレームワークは、失敗すべきテストを構築する[`#[should_panic]`属性](https://doc.rust-lang.org/rust-by-example/testing/unit_testing.html#testing-panics)をサポートしています。
例えば、不正な引数が渡されたときに関数が失敗することを検証するなど、これはとても便利です。
不運にも、それは標準ライブラリのサポートが必須であるため、`#[no_std]`において、この属性はサポートされていません。

我々のカーネルで`#[should_panic]`属性が使えないので、パニック・ハンドラから成功したエラーコードで終了する統合テストを作成することで、似たような振る舞いを得られます。
`should_panic`と名前を付けたそのようなテストの作成を始めましょう。

```rust
#![no_std]
#![no_main]

use blog_os::{exit_qemu, serial_println, QemuExitCode};
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);

    loop {}
}
```

このテストは、まだ`_start`関数やカスタム・テスト・ランナー属性を定義していないので未完成です。
不足している部分を追加しましょう。

```rust
// in tests/should_panic.rs

#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test();
        serial_println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}
```

我々の`lib.rs`から`test_runner`を再履ゆおうする代わりに、そのテストは、テストがパニックなしで戻ってきたときに失敗終了コードで終了する、独自の`test_runner`関数を定義しています（テストがパニックすることを望んでいます）。
もし、テスト関数が定義されていない場合、ランナーは成功エラーコードで終了します。
ランナーは常に1つのテストを実行した後に終了するため、1つ以上の`#[test_case]`関数を定義することは、意味がありません。

現時点で、失敗すべきテストを作成することができます。

```rust
use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[test_case]
fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
```

そのテストは`0`と`1`が等しいと主張する`assert_eq`を使用しています。
もちろん、これは誤りなので、望んだようにテストはパニックします。
我々は`Testable`トレイトを使用できないため、ここでは`serial_print!`を使用して関数の名前を手動で出力する必要があることに注意してください。

`cargo test --test should_panic`を通じてテストを実行したとき、予期したとおりテストがパニックしたので、成功を確認します。
我々が主張をコメントアウトして（`// assert_eq(0, 1);`）、再度テストすると、"test did not panic"メッセージでそれが確かに失敗することを確認できます。

この方法の重大な問題は、それが1つのテスト関数のときのみ機能することです。
複数の`#[test_case]`関数では、パニック・ハンドラが呼び出された後で、実行を継続することができないため、最初の関数のみ実行されます。
現在、この問題を解決する良い方法がわかりませんが、アイデアがあれば教えて下さい！

### 安全ベルトのないテスト

1つのテスト関数のみを持つ統合テスト（我々の`should_panic`テストのような）のために、本当はテスト・ランナーが必要ではありません。
このような場合において、完全にテスト・ランナーを無効にして、`_start`関数内のテストを直接実行できる。

これのキーは`Cargo.toml`内のテスト内の`harness`フラグを無効にすることで、それはテスト・ランナーが統合テストに使用されるかどうかを定義します。
それを`false`に設定したとき、デフォルトのテスト・ランナーとカスタムしたテスト・ランナーの機能の両方が無効になるので、テストは普通の実行形式のように取り扱われます。

`should_panic`テストのために、`harness`フラグを無効にしましょう。

```toml
// in Cargo.toml

[[test]]
name = "should_panic"
harness = false
```

現時点で、`test_runner`に関連するコードを削除することにより、`should_panic`テストをとても単純になります。
その結果はこのようになります。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);

    loop {}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);

    loop {}
}
```

現時点で、`_start`関数から直接`should_fail`関数を呼び出し、もしそれが戻ってきたとき失敗終了コードで終了しています。
現時点で、`cargo test --test should_panic`を実行したとき、テストが前と同じ振る舞いをすることを確認できます。

`should_panic`テストの作成から離れて、例えば、個々のテスト関数が副作用を持ち、特別な順番で実行する必要があるとき、`harness`属性を無効にすることは、複雑な統合テストで役に立ちます。

## まとめ

テストは、特定のコンポーネントが望まれている振る舞いを持っているかを保証するために、とくに役に立つ技術です。
もし、それらがバグがないことを示すことはできませんが、バグを見つけたり、特に退行を防ぐための役に立つツールです。

この投稿で、我々のRustカーネルのためのテスト・フレームワークを準備する方法を説明しました。
われわれのベア・メタル環境において、単純な`#[test_case]`属性のサポートを実装するために、Rustのカスタム・テスト・フレームワーク機能を使用しました。
QEMUで`isa-debug-exit`機器を使用することで、我々のテスト・ランナーが、テストを実行して、テストの状態を報告した後に、QEMUを終了することができます。
VGAバッファの代わりにコンソールにエラー・メッセージを出力するために、シリアル・ポートの基本的なドライバを作成しました。

我々の`println`マクロのためのいくつかのテストを作成した後、投稿の後半で統合テストを探求しました。
それらは`tests`ディレクトリに存在して、完全に分離した実行形式として扱われることを学びました。
それらに`exit_qemu`関数と`serial_println`マクロにアクセスできるようにすることで、すべての実行形式と統合テストによってインポートされるようにしたライブラリに、ほとんどのコードを移動しました。
統合テストがそれら独自の分離された環境で実行されるため、それらはハードウェアとの相互作用をテストできるようにして、またパニックすべきテストを作成できるようにしました。

現在、QEMUの内部の現実的な環境で実行するテスト・フレームワークを持っています。
これからの投稿でより多くのテストを作成することにより、我々のカーネルがより複雑になったときに、管理できるようにカーネルを保守できるように維持することができます。

## 次は何ですか？

次の投稿では、*CPU例外*を探求します。
0による除算や、マップされていないメモリ・ページ（"ページ・フォルト"と呼ばれます）へのアクセスなど、なんらか不正なことが発生したときに、これらの例外はCPUによって投げられます。
将来発生するエラーをデバッグするために、これらの例外を掴んで検査することができるようにすることは、これらの例外は非常に重要です。
例外処理は、キーボードをサポートするために要求されるハードウェア割り込みの処理ととても似ています。
