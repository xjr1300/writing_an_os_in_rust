# 自立したRustバイナリ

我々独自にオペレーティング・システム・カーネルを作成する最初のステップは、標準ライブラリとリンクしないRust実行形式を作成することです。
これは、オペレーティング・システムの基盤無しで、ベア・メタルでRustコードを実行できるようにします。

このブログは[GitHub](https://github.com/phil-opp/blog_os)で公開で開発されています。
もし、問題や質問がある場合は、ここで問題（issue）を発行（open）してください。
[下で](https://os.phil-opp.com/freestanding-rust-binary/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-01`](https://github.com/phil-opp/blog_os/tree/post-01)ブランチで見つけることができます。

## 導入

オペレーティング・システム・カーネルを記述するために、我々はオペレーティング・システムの機能に依存しないコードを必要とする。
これは、スレッド、ファイル、ヒープ、メモリー、ネットワーク、乱数、標準出力またはOSの抽象化や特別なハードウェアを要求する他の機能を使用することができないことを意味します。
我々独自のOSと独自のドライバーを作成することを試みているため、これは当然です。

これは我々が[Rust標準ライブラリ](https://doc.rust-lang.org/std/)のほとんどを使用できないことを意味しますが、我々がシユ王できるRustの機能はたくさんあります。
例えば、我々は[イテレーター](https://doc.rust-lang.org/book/ch13-02-iterators.html)、[クロージャー](https://doc.rust-lang.org/book/ch13-01-closures.html)、[パターン・マッチング](https://doc.rust-lang.org/book/ch06-00-enums.html)、[オプション](https://doc.rust-lang.org/core/option/)と[リザルト](https://doc.rust-lang.org/core/result/)、[文字書式](https://doc.rust-lang.org/core/macro.write.html)、そしてもちろん[所有権システム](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)を使用できます。
これらの機能は、[未定義な振る舞い](https://www.nayuki.io/page/undefined-behavior-in-c-and-cplusplus-programs)や[メモリ安全性](https://tonyarcieri.com/it-s-time-for-a-memory-safety-intervention)について心配することなく、とても表現豊かに、高レベルの方法で、カーネルを記述できるようにします。

RustでOSカーネルを作成するために、我々はオペレーティング・システムを基盤としない実行形式を作成することが必要です。
そのような実行形式は、よく「自立」または「ベア・メタル」実行形式と呼ばれます。

この投稿は自立したRustバイナリを作成する必要なステップとなぜそのステップが必要とされるのかを説明しています。
もし、単に最小限の例に興味を持っている場合は、[結果に移動](https://os.phil-opp.com/freestanding-rust-binary/#summary)することができます。

## 標準ライブラリの無効化

デフォルトでは、すべてのRustクレートは[標準ライブラリ](https://doc.rust-lang.org/std/)にリンクしており、それはスレッドや、ファイルまたはネットワークのような機能のために、オペレーティング・システムに依存しています。
それはC標準ライブラリの`libc`にも依存しており、それはOSサービスと緊密に相互作用しています。
我々の計画はオペレーティング・システムを記述することなので、任意のOS依存のライブラリを使用することができません。
よって、我々は[`no_std`属性](https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html)を介して標準ライブラリの自動抱合を無効にする必要があります。

我々は新しいcargoアプリケーション・プロジェクトを作成することで開始します。
これをする最も簡単な方法は、コマンドラインを介すことです。

```bash
cargo new blog_os --bin --edition 2018
```

プロジェクトに`blog_os`という名前を付けましたが、独自の名前を選択することができます。
`--bin`フラグは実行可能なバイナリ（対称的にはライブラリ）を作成することを指定しており、`--edition 2018`フラグは我々のクレートがRustの[2018エディション](https://doc.rust-lang.org/nightly/edition-guide/rust-2018/index.html)を使用することを指定しています。
コマンドを実行した時、cargoは我々のためにイアkのディレクトリ構成を作成します。

```
blog_os
├── Cargo.toml
└── src
    └── main.rs
```

`Cargo.toml`は、例えばクレートの名前、著者、[セマンティック・バージョン](https://semver.org/)番号そして依存関係など、クレートの設定を含んでいます。
`src/main.rs`ファイルは我々のクレートのルートモジュールと`main`関数を含んでいます。
`cargo build`を介してクレートをコンパイルすることができ、そして`target/debug`サブフォルダ内にコンパイルされた`blog_os`バイナリを実行できます。

### The `no_std` Attribute

現在、我々のクレートは標準ライブラリに暗黙的にリンクしています。
[`no_std`属性](https://doc.rust-lang.org/1.30.0/book/first-edition/using-rust-without-the-standard-library.html)を追加することにより、これを無効にしましょう。

```rust
// main.rs
#![no_std]

fn main() {
    println!("Hello, world!");
}
```

（`cargo build`）を実行することにより）、今ビルドを試みた時、以下のエラーが発生します。

```
error: cannot find macro `println!` in this scope
 --> src/main.rs:4:5
  |
4 |     println!("Hello, world!");
  |     ^^^^^^
```

このエラーの理由は[`println`マクロ](https://doc.rust-lang.org/std/macro.println.html)は標準ライブラリの一部で、我々はもはやインクルードしていません。
よって、我々はもはや`モノ(things)`を出力（print)できません。
`println`は、オペレーティング・システムによって提供される特別なファイル・ディスクリプタである[標準出力](https://en.wikipedia.org/wiki/Standard_streams#Standard_output_.28stdout.29)に書き出すため、当然です。

よって、印字を削除して、空のmain関数で再度試みましょう。

```rust
// main.rs
#![no_std]

fn main() {}
```

```
> cargo build
error: `#[panic_handler]` function required, but not found
error: language item required, but not found: `eh_personality`
```

たった今、コンパイラは`#[panic_handler]`関数と*言語アイテム*が無いことをエラーとして出力しました。

## パニックの実装

`panic_handler`属性は、[panic](https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html)が発生したときにコンパイラが呼び出すべき関数を定義します。
標準ライブラリはそれ独自のパニック・ハンドラ関数を提供しますが、`no_std`環境において、我々は我々自身でそれを定義する必要があります。

```rust
// in main.rs
use core::panic::PanicInfo;

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

[`PanicInfo`パラメーター](https://os.phil-opp.com/freestanding-rust-binary/#:~:text=The-,PanicInfo%20parameter,-contains%20the%20file)は、パニックが発生したファイルと行と、オプションでパニック・メッセージを含んでいます。
この関数は、決して戻るべきではないので、["never"型](https://doc.rust-lang.org/nightly/std/primitive.never.html) `!`を返却することで、それを[発散関数](https://doc.rust-lang.org/1.30.0/book/first-edition/functions.html#diverging-functions)としてマークされます。
現在のところ、この関数内で我々がすることができる良い方法が無いので、我々は単に永遠にループします。

## `eh_personality`言語アイテム

言語アイテムは、コンパイラによって内部的に要求される特別な関数と型で、最後の手段としてのみ実行するべきです。
その理由は、言語アイテムは詳細な実装が非常に不安定で、型チェックすらされていないためです（よって、コンパイラは、関数が正しい引数の型を持っているかさえチェックしません）。
幸運にも、上記言語アイテムエラーを修正するより安定的な方法があります。

[`eh_personality`言語アイテム](https://github.com/rust-lang/rust/blob/edb368491551a77d77a48446d4ee88b35490c565/src/libpanic_unwind/gcc.rs#L11-L45)は、[スタック巻き戻し](https://www.bogotobogo.com/cplusplus/stackunwinding.php)を実装するために使用される関数をマークします。
デフォルトでは、Rustは、[パニック](https://doc.rust-lang.org/stable/book/ch09-01-unrecoverable-errors-with-panic.html)が発生した場合に、すべての生存するスタック変数のデストラクタを実行するために巻き戻しを使用します。
これは、すべての使用さたメモリが開放され、親スレッドがパニックを受け取って実行を継続することを保証します。
しかし、巻き戻しは複雑な処理で、OSの特別なライブラリ（例えば、Linuxでは[libunwind](https://www.nongnu.org/libunwind/)、Windowsでは[構成された例外ハンドリング](https://docs.microsoft.com/en-us/windows/win32/debug/structured-exception-handling)など）を要求するので、我々のオペレーティング・システムではそれを使用しません。

### 巻き戻しの無効化

巻き戻しが望まれない他のユースケースがあるので、Rustは代わりに[パニックでアボート](https://github.com/rust-lang/rust/pull/32900)するオプションを提供しています。
これは、巻き戻しのシンボル情報の生成を無効化して、それによりかなりバイナリサイズを減らします。
巻き戻しを無効化する複数の場所があります。
最も簡単な方法は`Cargo.toml`に以下の行を追加することです。

```toml
[profile.dev]
panic = "abort"

[profile.release]
panic = "abort"
```

これは、`dev`プロフファイル（`cargo build`で使用される）と`release`プロファイル（`cargo build --release`で使用される）の両方のパニック戦略を`abort`に設定します。
現在、`eh_personality`言語アイテムはもはや要求されません。

現在、上記両方のエラーを修正しました。
しかしながら、今、それのコンパイルを試みた場合、他のエラーが発生します。

```
> cargo build
error: requires `start` lang_item
```

我々のプログラムは`start`言語アイテムがなく、それはエントリ・ポイントを定義します。

## `start`属性

`main`関数は、プログラムが実行された時に、最初に呼ばれる関数だと考えるかもしれません。
しかしながら、ほとんどの言語は[ランタイム・システム](https://en.wikipedia.org/wiki/Runtime_system)を持っており、それはガベージ・コレクション（例えばJavaにおいて）またはソフトウェア・スレッド（例えばGoにおけるゴールーチン）のようなものの責任を持っています。
このランタイムは`main`の前に呼び出される必要があるので、それは自分自身で初期化する必要があります。

標準ライブラリにリンクする典型的なRustバイナリにおいて、実行は`crt0`（"Cランタイム・ゼロ）と呼ばれるCランタイム・ライブラリを開始して、それはCアプリケーションのために環境を準備する。
これはスタックの作成や正しいレジスタ内に引数を配置することが含まれます。
次にCランタイムは`start`言語アイテムによってマークされた、[Rustランタイムのエントリ・ポイント](https://github.com/rust-lang/rust/blob/bb4d1491466d8239a7a5fd68bd605e3276e97afb/src/libstd/rt.rs#L32-L73)を呼び出します。
Rustは非常に小さなランタイムのみ持っており、それはスタック・オーバーフロー・ガート、またパニックでバックトレースを出力（print）する準備をします。
このランタイムは最終的に`main`関数を呼び出します。

我々の自立した実行形式はRustランタイムと`crt0`へのアクセスを持たないので、独自のエントリポイントを定義する必要があります。
`start`言語アイテムの実装は、`crt0`が必要なので役に立ちません。
代わりに、直接`crt0`エントリ・ポイントを上書きする必要があります。

### エントリ・ポイントの上書き

通常のエントリ・ポイント・チェインを使用しないことをRustコンパイラに伝えるために、`#![no_main]`属性を追加します。

```rust
#![no_std]
#![no_main]

use core::panic::PanicInfo;

/// この関数はパニックしたときに呼び出されます。
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
  loop {}
}
```

あなたは`main`関数を削除したことに気づいたかもじれません。
その理由は、`main`はそれを呼び出す基盤となるランタイムがないと意味がないからです。
代わりに、独自の`_start`関数でオペレーティング・システムのエントリ・ポイントを上書きします。

```rust
#[no_mangle]
pub extern "C" fn _start() -> ! {
  loop {}
}
```

`#no_mangle]`属性を使用することによって、Rustコンパイラが本当に関数を`_start`という名前で出力することを保証するために、[名前難読化](https://en.wikipedia.org/wiki/Name_mangling)を無効にします。
その属性なしのとき、コンパイラはすべての関数に一意な名前を与えるために、`_ZN3blog_os4_start7hb173fedf945531caE`のような不可解なシンボルを生成します。
その属性は、次のステップでリンカにエントリ・ポイント関数の名前を伝える必要があるため、必要とされます。

また、コンパイラにそれが[C呼び出し規約](https://en.wikipedia.org/wiki/Calling_convention)（未定義のRust呼び出し規約の代わりに）を使用していることを伝えるために、`extern "C"`として関数をマークする必要があります。
関数に`_start`という名前を付けた理由は、ほとんどのシステムにとって、これがデフォルトのエントリ・ポイント名であるからです。

`!`返却型は、関数が発散することを意味しており、例えば、戻ることが許されません。
これは、オペレーティング・システムやブートローダによって直接呼び出されますが、任意の関数によってエントリポイントが呼び出されないため必要とされます。
よって返却する代わりに、エントリ・ポイントは、例えばオペレーティング・システムの[`exit`システム・コール](https://en.wikipedia.org/wiki/Exit_(system_call))を呼び出すべきです。
我々の場合、自立したバイナリが返却したら何かすることが残されていなため、マシンをシャットダウンすることは妥当な動作であるかもしれません。
今のところ、永遠にループすることで要件を満たしています。

現段階で1cargo buid`を実行したとき、見苦しい*リンカ・エラー*を得ます。

## リンカ・エラー

リンカは生成されたコードを実行形式に結合するプログラムです。
実行形式のフォーマットは、Linux、WindowsそしてmacOSで異なるため、それぞれのシステムは異なるエラーをスローするそれ独自のリンカを持っています。
エラーの根本的な原因は同じです。
リンカのデフォルト設定は、我々のプログラムが依存していないCランタイムに依存していることを想定しています。

エラーを解決するために、リンカにそれがCランタイムを含めないように伝える必要があります。
これをするためには、特定の引数のセットをリンカにわたすが、ベア・メタル・ターゲット用にビルドする必要があります。

### ベア・メタル用のビルド

デフォルトでRustは現在のシステム環境で実行できるように実行形式をビルドすることを試みます。
例えば、`x86_64`上のWindowsを使用している場合、Rustは`x86_64`命令を使用したWindows実行形式である`.exe`をビルドすることを試みます。
この環境は"ホスト"システムと呼ばれています。

異なる環境を指定（describe）するために、Rustは[*ターゲット・トリプル*](https://clang.llvm.org/docs/CrossCompilation.html#target-triple)と呼ばれる文字列を使用します。
`rustc --version --verbose`を実行することで、ホスト・システムのターゲット・トリプルを確認できます。

```
rustc 1.35.0-nightly (474e7a648 2019-04-07)
binary: rustc
commit-hash: 474e7a6486758ea6fc761893b1a49cd9076fb0ab
commit-date: 2019-04-07
host: x86_64-unknown-linux-gnu
release: 1.35.0-nightly
LLVM version: 8.0
```

上記の出力は`x86_64`Linuxシステムからのものです。
`host`トリプルは`x86_64-unknown-linux-gnu`で、CPUアーキテクチャ（`x86_64`）、ベンダー（`unknown`）、オペレーティング・システム（`linux`）そして[ABI](https://en.wikipedia.org/wiki/Application_binary_interface)（`gnu`）を含んでいます。

> `ABI`: バイナリ・レベルでメソッドなどを含む関数やクラスなどを含むデータの仕様を規定したもので、エンディアンや型のサイズを意識したもの。
> ABIが同じで異なるOSがあった場合、一方のOSで動作するプログラムは、変更なしで他方のOSので実行できる。

ホスト・トリプル用にコンパイルすることにより、Rustコンパイラとリンカは、デフォルトでCランタイムを使用するLinuxやWindowsなど基盤となるオペレーティング・システムがあることを想定し、リンカ・エラーを引き起こします。
よって、リンカ・エラーを回避するために、基盤となるオペレーティング・システムのない異なる環境用にコンパイルします。

そのようなベア・メタル環境の例は`thumbv7em-none-eabihf`ターゲット・トリプルで、それは[埋め込み](https://en.wikipedia.org/wiki/Embedded_system) [ARM](https://en.wikipedia.org/wiki/ARM_architecture)システムを示します。
詳細は重要ではなく、重要な全てはターゲット・トリプルがオペレーティング・システム基盤を持たないことで、それはターゲット・トリプルの中の`none`によって示されています。
このターゲット用にコンパイルするために、rustupにそれを追加します。

```bash
rustup target add thumbv7em-none-eabihf
```

これは、そのシステム用の標準（そしてコア（核））ライブラリのコピーをダウンロードします。
これで、このターゲット用に我々の自立した実行形式にビルドできます。

```bash
cargo build --target thumbv7em-none-eabihf
```

`--target`引数を渡すことにより、ベアメタル用システム向けの我々の実行形式に[クロス・コンパイル](https://en.wikipedia.org/wiki/Cross_compiler)します。
ターゲット・システムはオペレーティング・システムを持たないので、リンカはCランタイムにリンクすることをやめて、リンカ・エラーなしでビルドが成功します。

これは、我々のOSカーネルを構築するために使用できる方法です。
`thumbv7em-none-eabihf`の代わりに、`x86_64`ベア・メタル環境し示す[カスタム・ターゲット](https://doc.rust-lang.org/rustc/targets/custom.html)を使用する予定です。
詳細は次の投稿で説明する予定です。

### リンカ引数

ベア・メタル・システム用にコンパイルする代わりに、隣家に特定の引数のセットを与えることで、リンカ・エラーを解決することもできます。
これは、我々のカーネルで使用する方法ではありませんので、このセクションのは任意で、完全を期すためにのみ提供されています。
任意のコンテンツを表示するために、下の *"リンカ引数"* をクリックしてください。

## まとめ

最小限の自立したRustバイナリはこのように見えます。

```rust
// src/main.rs
#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
```

```.toml
[package]
name = "crate_name"
version = "0.1.0"
authors = ["Author Name <author@example.com>"]

# the profile used for `cargo build`
[profile.dev]
panic = "abort" # disable stack unwinding on panic

# the profile used for `cargo build --release`
[profile.release]
panic = "abort" # disable stack unwinding on panic
```

バイナリをビルドするために、`thumbv7em-none-eabihf`のような、ベア・メタル・ターゲット用にコンパイルする必要があります。

```bash
cargo build --target thumbv7em-none-eabihf
```

あるいは、追加のリンカ引数を与えることで、ホストシステム用にそれをコンパイルできます。

```bash
# Linux
cargo rustc -- -C link-arg=-nostartfiles
# Windows
cargo rustc -- -C link-args="/ENTRY:_start /SUBSYSTEM:console"
# macOS
cargo rustc -- -C link-args="-e __start -static -nostartfiles"
```

これは単に自立したRustバイナリの最小限の例であることに注意してください。
例えば、スタックが`_start`関数が呼び出されたときにスタックが初期化されるなど、このバイナリは様々なことを予期しています。
よって、このようなバイナリを実際に使用するには、さらに多くの手順が必要です。
