# 最小のRustカーネル

この投稿では、x86アーキテクチャの最小の64bitRustカーネルを作成します。
前の投稿の[freestanding Rust binary](https://os.phil-opp.com/freestanding-rust-binary/)を基に、画面に何かを出力する起動可能なディスク・イメージを作成します。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/minimal-rust-kernel/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-02`](https://github.com/phil-opp/blog_os/tree/post-02)ブランチで見つけることができます。

## ブート・プロセス

コンピューターの電源を投入したとき、それはマザーボードの[ROM](https://en.wikipedia.org/wiki/Read-only_memory)に記録されたファームウェア・コードの実行を開始します。
このコードは[起動時自己テスト](https://en.wikipedia.org/wiki/Power-on_self-test)を実行して、利用可能なRAMを検出して、CPUとハードウェアを事前に初期化します。
その後、それは起動可能なディスクを探して、オペレーティング・システム・カーネルの起動を開始します。

x86において、２つのファームウェア標準があり、それは"Basic Input/Output System"（[BIOS](https://en.wikipedia.org/wiki/BIOS)）と新しい"Unified Extensible Firmware Interface"（[UEFI](https://en.wikipedia.org/wiki/Unified_Extensible_Firmware_Interface)）です。

BIOS標準は古く、時代遅れですが、単純で1980年代から任意のx86マシンで十分にサポートされています。
対称的にUEFIは、より現代的でより多くの機能を持っていますが、（少なくとの私の意見としては）準備することがより複雑です。

現在、我々はBIOSのサポートのみ提供していますが、UEFIのサポートを計画中です。
もしこれの手助けをしたいと思うのであれば、[GitHub issue](https://github.com/phil-opp/blog_os/issues/349)を確認してください。

### BIOSの起動

模倣されたBIOS使用する新しいUEFIに基づくマシンを含む、ほとんどすべてのx86システムはBIOS起動をサポートしています。
前世紀のすべてのマシンで同じブート・ロジックを使用できるため、これは素晴らしいことです。
しかし、この互換性の広さは、同時にBIOSブートの最大のデメリットでもあり、80年代の古めかしいブートローダがまだ動くように、CPUをリアルモードという16ビット互換モードにしてからブートすることを意味します。

しかし、最初から始めましょう。

コンピュータの電源を入れたとき、それはマザーボード上に配置された何らか特別なフラッシュ・メモリからBIOSをロードします。
BIOSは自己テストとはドウェアの初期化ルーチンを実行して、次にブータブル・ディスクを探します。
もし見つかった場合、制御はそのディスクの開始位置に保存された実行可能なコードの512バイト部分である*ブート・ローダー*に移されます。
ほとんどのブートローダーは512バイトより大きいため、ブートローダーは通常512バイトに収まる小さな第1段階と、続けて第1段階によってロードされる第2段階に分割されます。

ブートローダーはディスク上のカーネル・イメージの位置を決定して、それをメモリにロードする必要があります。
また、最初の16bit[リアル・モード](https://en.wikipedia.org/wiki/Real_mode)から、32bit[保護モード](https://en.wikipedia.org/wiki/Protected_mode)に、次に64bitレジスタとメイン・メモリ全体を利用できる64-bit[ロング・モード](https://os.phil-opp.com/minimal-rust-kernel/#:~:text=the%2064%2Dbit-,long%20mode,-%2C%20where%2064%2Dbit)に切り替える必要があります。
その3番めの仕事は、BIOSから特定の情報（メモリ・マップなど）を問い合わせ、それをOSカーネルに渡すことです。

ブートローダを記述することは、アセンブリ言語と「この魔法の値をこのプロセッサ・レジスタに書き込む」など、洞察力のない多くの手順を必要とするため、少し面倒です。
よって、この投稿ではブートローダーの作成を取り上げず、代わりにブートローダーをカーネルに自動的に付与する[ブートイメージ](https://github.com/rust-osdev/bootimage)という名前のツールを提供します。

もし、独自のブートローダーの高チックに興味があるのであれば、このトピックに関する投稿のセットが計画されています。

### マルチブート標準

すべてのオペレーティング・システムが1つのOSとのみ互換性のある独自のブートローダを実装することを避けるために、[フリー・ソフトウェア財団](https://en.wikipedia.org/wiki/Free_Software_Foundation)は1995年に[マルチブート](https://wiki.osdev.org/Multiboot)と呼ばれるオープンなブートローダーを作成しました。
その標準はブートローダーとオペレーティング・システム間のインターフェースを定義しているため、任意のマルチブートに互換性のあるブートローダは、陰萎のマルチブートローダーに互換性のあるオペレーティング・システムをロードできる。
そのリファレンス実装は[GNU GRUB](https://en.wikipedia.org/wiki/GNU_GRUB)であり、それはLinuxシステムにとって最も人気のあるブートローダーである。

マルチブートに準拠しているカーネルを作成するためには、[マルチブート・ヘッダ](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#OS-image-format)をカーネル・ファイルの先頭に挿入するだけです。
これはGRUBからOSをブートすることをとても簡単にします。
しかしGRUBとマルチブート標準はいくつかの問題を持っている。

* それらは32bit保護モードのみをサポートしています。これは64bitロング・モードにスイッチするようにCPUを設定する必要があること意味します。
* それらはカーネルの代わりにブートローダの作成を簡単にするために設計されました。
  例えば、カーネルが[調整されたデフォルトのページサイズ](https://wiki.osdev.org/Multiboot#Multiboot_2)にリンクする必要があり、そうしないとGRUBがマルチブート・ヘッダを見つけられません。
  他の例は、カーネルんき渡される[ブート情報](https://www.gnu.org/software/grub/manual/multiboot/multiboot.html#Boot-information-format)に、明確な抽象化を提供する代わりに、アーキテクチャに依存する構造が多く含まれていることです。
* GRUBとマルチブート標準の両方は、まばらにしかドキュメント化されていません。
* カーネル・ファイルから起動可能なディスク・イメージを作成するためには、ホスト・システムにGRUBをインストールする必要があります。
  これにより、WindowsまたはMacでの開発がより困難になることです。

これらの欠点により、我々はGRUBやマルチブート標準を使用しないことを決めました。
しかしながら、我々の[ブートイメージ](https://github.com/rust-osdev/bootimage)ツールにマルチブートのサポートを追加することを計画おり、それはカーネルをGRUBシステムにもロードできます。
マルチブート準拠のカーネルの作成に興味がある場合は、このブログ・シリーズの[初版](https://os.phil-opp.com/edition-1/)をチェックしてください。

### UEFI

（現在、我々はUEFIサポートを提供していませんが、喜んでサポートします。協力して貰える場合は[GitHub issue](https://github.com/phil-opp/blog_os/issues/349)で知らせてください。）

## 最小限のカーネル

現在、我々はどのようにコンピュータがブートするかを大まかに知り、我々独自の最小限のカーネルを作成する時間です。
我々のゴールは、ブートしたときに"Hello World!"をスクリーンに出力するディスク・イメージを作成することです。
我々は、前の投稿である[feestanding Rust binary](https://os.phil-opp.com/freestanding-rust-binary/)を拡張することにより、これを実施します。

覚えているかもしれませんが、我々は`cargo`を介して独立したバイナリを構築しましたが、オペレーティング・システムに依存しており、我々は異なるエントリ・ポイントの名前とコンパイル・フラグを必要としました。
それは、例えばあなたが実行しているシステムのように、`cargo`がデフォルトで*ホスト・システム*向けにビルドすることが理由です。
Windowsなどで実行されるカーネルはあまり意味がないため、これは我々のカーネルにとって望ましいことではありません。
代わりに、明確に定義された*ターゲット・システム*用にコンパイルしたいと考えています。

### Rustナイトリのインストール

Rustは安定版、ベータ版そしてナイトリ版の3つのリリース・チャネルを持っています。
The Rust Bookは3つのチャネルの違いをとても良く説明していので、時間を取ってそれを[確認](https://doc.rust-lang.org/book/appendix-07-nightly-rust.html#choo-choo-release-channels-and-riding-the-trains)してください。
オペレーティング・システムを構築するために、我々はナイトリ・チャネルでのみリユ出来るいくつかの実験的な機能を必要とするので、Rustのナイトリ版をインストールする必要があります。

Rustのインストールを管理するために、[rustup](https://www.rustup.rs/)を強く推奨します。
それは、ナイトリ版、ベータ版そして安定版のコンパイラを並べてインストールしてくれて、それらを簡単に更新できるようにします。
rustupで、カレント・ディレクトリで`rustup override set nightly`を実行することで、ナイトリ・コンパイラを使用でいるようになります。
あるいは、プロジェクトのルート・ディレクトリに、`nightly`コンテンツを持つ`rust-toolchain`と呼ばれるファイルを追加できます。
`rustc --verion`を実行することで、ナイトリ版がインストールされているか確認できます。
そのバージョン番号の末尾に`-nightly`が含まれていなければなりません。

ナイトリ・コンパイラは、我々のファイルの最も丈夫に*機能フラグ*を使用することにより、様々な実験的な機能をオプト・インできます。
例えば、我々の`main.rs`の最上部に`E#![feature(asm)]`を追加することで、実験的な[`asm!マクロ`](https://doc.rust-lang.org/stable/reference/inline-assembly.html)を有効にできます。
そのような実験的機能は完全に不安定で、それは将来のRustのバージョンが、事前に警告なしで、それらを変更または削除するかもしれないことを意味することに注意してください。
この理由で、我々はそれらが絶対に必要なときのみ使用する予定です。

### ターゲット仕様

Cargoは`--target`パラメータを介して異なるターゲット・システムをサポートします。
そのターゲットは[`ターゲット・トリプル`](https://clang.llvm.org/docs/CrossCompilation.html#target-triple)で指定され、それはCPUアーキテクチャ、ベンダー、オペレーティング・システムそして[ABI](https://stackoverflow.com/a/2456882)を示しています。
例えば、`x86_64-unkinown-linux-gnu`ターゲット・トリプルは、`x86_64`CPUのシステム、不明のベンダー、そしてGNU ABIを持つLinuxオペレーティング・システムを示します。
RustはAndroidの`arm-linux-androidabi`や[WebAssemblyの`wasm32-uniknown-unknown`](https://www.hellorust.com/setup/wasm-target/)を含む、多くの異なる[ターゲット・トリプル](https://forge.rust-lang.org/release/platform-support.html)をサポートしています。

しかしながら、我々のターゲット・システム用に、我々はいくつかの特別な設定パラメータ（例えば、OSを基礎としない）を必要とするので、[存在するターゲット・トリプル](https://forge.rust-lang.org/release/platform-support.html)で適合するものがありません。
幸運にも、RustはJSONファイルを通じて[我々独自のターゲット](https://doc.rust-lang.org/nightly/rustc/targets/custom.html)を定義できます。
例えば、`x86_64-unknown-linux-gnu`ターゲットを指定するJSONファイルは、このようになります。

```json
{
    "llvm-target": "x86_64-unknown-linux-gnu",
    "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
    "arch": "x86_64",
    "target-endian": "little",
    "target-pointer-width": "64",
    "target-c-int-width": "32",
    "os": "linux",
    "executables": true,
    "linker-flavor": "gcc",
    "pre-link-args": ["-m64"],
    "morestack": false
}
```

ほとんどのフィールドは、そのプラットフォーム用のコードを生成するために、LLVMから要求されます。
例えば、[`data-layout`]()フィールドは様々な整数、浮動小数点数、そしてポインタの型のサイズを定義しています。
そして、`target-pointer-width`のような、Rustが条件付きコンパイルで使用するために使用するフィールドがあります。
3番目の種類のフィールドは、どのようにクレートがビルドされるべきかを指定しています。
例えば、`pre-link-args`フィールドは[リンカ](https://en.wikipedia.org/wiki/Linker_(computing))に渡す引数を指定しています。

我々はカーネルで`x86_64`システムをターゲットにするので、我々のターゲットの指定は、上と非常によく似ています。
一般的なコンテンツを持つ`x86_64-blog_os.json`ファイル（好みの任意の名前を選択できます）を作成することから始めましょう。

```json
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true
}
```

ベア・メタルで実行するため、`llvm-target`フィールドのOSと、`os`フィールドを`none`に変更していることに注意してください。

以下のビルド関連のエントリを追加します。

```
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
```

プラットフォーム・デフォルトのリンカ（Linuxターゲットをサポートしていないかもしれません）を使用する代わりに、我々のカーネルをリンクするためにRustに同梱されているクロス・プラットフォームの[LLD](https://lld.llvm.org/)リンカを使用します。

```
  "panic-strategy": "abort",
```

この設定は、ターゲットがパニックが発生したときに、[スタックの巻き戻し](https://www.bogotobogo.com/cplusplus/stackunwinding.php)をサポートしないことを指定しており、代わりにプログラムは直接アボートするべきです。
これは、`Cargo.toml`内の`panic = "abort"`と同じ効果を持つので、そこからそれを削除できます。
（Cargo.tomlオプションとは対照的に、このターゲット・オプションは、この記事の後半で`core`ライブラリを再コンパイルするときにも適用されることに注意してください。そのため、Cargo.tomlオプションを保持したい場合でも、必ずこのオプションを含めてください。）

```
  "disable-redzone": true,
```

カーネルを記述しているため、任意のポイントで割り込みを対処する必要があります。
それを安全に行うために、*"レッド・ゾーン"*と呼ばれる特定のスタック・ポインタの最適化を不幸にする必要があります。
そうしなければ、それはスタックの破損を引き起こします。
詳細な情報は、[レッド・ゾーンの無効化](https://os.phil-opp.com/red-zone/)についての別の投稿を参照してください。

```
  "features": "-mmx,-sse,+soft-float",
```

`features`フィールドは目的の機能を有効／無効にします。
`mmx`と`sse`機能をマイナスをそれらの前に付けることによって無効化して、`soft-float`機能をプラスをその前に付けることによって有効にしています。
それぞれのフラグの間にスペースがあってはならないことに注意してください。

`mmx`と`sse`機能は[シングル・インストラクション・マルチ・データ（SIMD）](https://en.wikipedia.org/wiki/SIMD)命令をサポートするかを決定します。それはよくプログラムの実行速度を顕著に上げます。
しかしながら、OSカーネルで大きなSIMDレジスタを使用すると、パフォーマンスの問題が発生します。
その理由は、カーネルは割り込まれたプログラムを継続する前に、すべてのレジスタにそれらのオリジナルな状態に戻す必要があるからです。
これは、カーネルはシステム・コールやハードウェア割り込みが発生するたびに、完全なSIMDの状態をメイン・メモリに保存する必要があることを意味します。
SIMDの状態はとても大きく（512-1600bytes）そして、割り込みはとても頻繁に発生する可能性があり、これらの追加的な保存／リストア操作はかなりパフォーマンスが劣化します。
これを避けるために、我々のカーネルはSIMDを無効にしました（not for application running on top!）。

SIMDを無効にする問題は、`x86_64`における浮動小数点演算は、デフォルトでSIMDレジスタを必要とすることです。
この問題を解決するために、`soft-float`機能を追加して、通常の整数に基づくソフトウェア関数を通じて、すべての浮動小数点演算を模倣します。

詳細な情報は、我々の投稿である[SIMDの無効化](https://os.phil-opp.com/disable-simd/)を参照してください。

### まとめ

現在、我々のターゲット仕様ファイルはこのとおりです。

```json
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float"
}
```

## カーネルのビルド

新しいターゲットでのコンパイルはLinux規約を使用します（理由はよくわかりません。LLVMのデフォルトだと考えています）。
これは、[前の投稿](https://os.phil-opp.com/freestanding-rust-binary/)で説明した`_start`と命名したエントリポイントが必要であることを意味します。

ホストOSに関わらず、エントリ・ポイントを`_start`と呼ばれる必要があることに注意してください。

現在、`--target`でJSONファイルの名前を渡すことにより、新しいターゲットでアーネルをビルドできます。

```
> cargo build --target x86_64-blog_os.json
error[E0463]: can't find crate for `core`
```

失敗しました。
エラーはRustコンパイラが[`core`ライブラリ](https://doc.rust-lang.org/nightly/core/index.html)を見つけられないことを伝えます。
このライブラリは`Result`、`Option`そしてイテレータなどの基本的なRustの型を含み、暗黙的にすべての`no_std`クレートにリンクされます。

その問題は、コア／ライブラリが*事前にコンパイルされた*ライブラリとしてRustコンパイラと一緒に配布されることです。
よって、それはサポートされているホスト・トリプル（例えば`x86_64-unknown-linux-gnu`）に対してのみ有効で、カスタムターゲットに対しては無効です。
他のターゲット用にコードをコンパイルする場合、最初にこれらのターゲット用に`core`を再コンパイルする必要があります。

### `build-std`オプション

そこでcargoの[`build-std`機能]の出番です。
それは、Rustのインストールで同梱された事前コンパイルされたバージョンを使用する代わりに、`core`と要求に応じて他の標準ライブラリ・クレートを再コンパイルします。
この機能はとても新しく、未だ完了していないので、"unstable"とマークして、[ナイトリRustコンパイラ](https://os.phil-opp.com/minimal-rust-kernel/#installing-rust-nightly)でのみ利用できます。

その機能を使用するために、我々は次の内容を含む`.cargo/config.toml`を[cargo設定](https://doc.rust-lang.org/cargo/reference/config.html)ファイルを作成する必要がある。

```toml
# in .cargo/config.toml
[unstable]
build-std = ["core", "compiler_builtins"]
```

これはcargoに、cargoが`core`と`compiler_builtins`ライブラリを再コンパイルする必要があることを指示しています。
後者は、それが`core`の依存があるため必要です。
これらのライブラリを再コンパイルするために、cargoはrustのソースコードにアクセスする必要があるので、それは`rustup component add rust-src`でインストール出来る。

> 注意: `unstable.build-std`設定キーは、少なくとも2020-07-15からのRustナイトリを要求します。

`unstable.build-std`設定キーを設定して、`rust-src`コンポーネントをインストールした後、ビルド・コマンドを再実行できます。

```
> cargo build --target x86_64-blog_os.json
   Compiling core v0.0.0 (/…/rust/src/libcore)
   Compiling rustc-std-workspace-core v1.99.0 (/…/rust/src/tools/rustc-std-workspace-core)
   Compiling compiler_builtins v0.1.32
   Compiling blog_os v0.1.0 (/…/blog_os)
    Finished dev [unoptimized + debuginfo] target(s) in 0.29 secs
```

`cargo build`が`core`、`rust-std-workspace-core`（`compiler_builtins`の依存）、そして`compiler_builtins`ライブラリが我々のカスタム・ターゲットのために再コンパイルしたのを見ました。

#### メモリ関連の組み込み関数

Rustコンパイラは特定のビルトインされた関数のセットがすべてのシステムで利用出来ることを想定しています。
これらの大半の関数は、我々がちょうど再コンパイルした`compiler_builtins`クレートによって提供されています。
しかし、そのクレート内のいくつかのメモリ関連の関数がありますが、それらはデフォルトで有効ではありません。なぜなら、それらは通常システムのCライブラリによって提供されるからです。
これらの関数は`memset`を含んでおり、それは与えられた値でメモリ・ブロックのすべてのバイトを設定して、`memcpy`はあるメモリ・ブロックを他にコピーして、`memcmp`は2つのメモリ・ブロックを比較します。
現在、我々のカーネルをコンパイルするためにこれらの関数を使用する必要がありませんが、それにいくらかコードを追加するとすぐに必要になります（例えば、構造体をコピーするとき）。

オペレーティング・システムのCライブラリにリンクできないので、コンパイラにこれらの関数を提供する別の方法が必要になります。

この1つの可能性のある方法は、独自の`memset`などの関数を実装して、それらに`#[no_mangle]`属性を適用することです（コンパイル中の自動命名を避けるために）。
しかし、これらの関数の実装におけるほんの少しのミスが、未定義な振る舞いを導き出す可能性があるため危険です。
例えば、`for`ループで`memcpy`を実装することは、`for`ループが暗黙的に[`IntoIterator::into_iter`](https://doc.rust-lang.org/stable/core/iter/trait.IntoIterator.html#tymethod.into_iter)トレイト・メソッドを呼び出し、それが`memcpy`を再び呼び出すかもしれないので、永遠に繰り返す結果になるかもしれません。
よって、代わりに存在する十分にテストされた実装を再利用することは良い考えです。

幸運にも、`compiler_builtins`クレートは既に必要なすべての関数の実装を含んでおり、それらは、Cライブラリの実装と衝突しないように、デフォルトで無効になっているだけです。
我々はcargoの[`build-std-features`](https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#build-std-features)フラグを`["compiler-builtins-mem"]`にセットすることにより、それらを有効にできます。
`build-std`フラグと同様に、このフラグは`-Z`フラグとしてコマンドに渡すか、`.cargo/config.toml`ファイルの`unstable`テーブル内で設定できます。
このフラグでビルドすることを常に望むため、我々にとって設定ファイル・オプションがより理にかなっています。

```toml
# in .cargo/config
[unstable]
build-std-features = ["compiler-builtins-mem"]
build-std = ["core", "compiler-builtins"]
```

（`compiler-builtins-mem`機能のサポートは、[とても最近に追加された]()ので、そのために少なくともRustナイトリの`2020-09-30`を必要とします。）

舞台裏では、このフラグは`compiler_builtins`クレートの[`mem`機能](https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/Cargo.toml#L54-L55)を有効にします。
この効果は`#[no_mangle]`属性がクレートの[`memcpyなどの実装](https://github.com/rust-lang/compiler-builtins/blob/eff506cd49b637f1ab5931625a33cef7e91fbbf6/src/mem.rs#L12-L69)に適用され、リンカがそれらを利用できるようにすることです。

この変更で、我々のカーネルはすべてのコンパイラが必要とする関数の妥当な実装をもち、それによって、それは我々のコードがより複雑になってもコンパイルすることを継続するでしょう。

#### デフォルト・ターゲットの設定

すべての`cargo build`の呼び出しで`--target`パラメーターを渡すことを避けるために、デフォルトターゲットを上書きできます。
これをするために、[cargo設定ファイル](https://doc.rust-lang.org/cargo/reference/config.html)である`.cargo/config.toml`に次を追加します。

```toml
# in .cargo/config.toml

[build]
target = "x86_64-blog_os.json"
```

これは、明示的な`--target`引数がないとき、`x86_64-blog_os.json`ターゲットを使用することを`cargo`に伝えます。
これは、現在、我々のカーネルを単に`cargo build`でビルドできることを意味します。
cargoの設定オプションの情報をより得たい場合は、[公式ドキュメント](https://doc.rust-lang.org/cargo/reference/config.html)を確認してください。

現在、、単に`cargo build`することで、ベアメタル・ターゲット用に我々のカーネルをビルドできます。
しかしながら、ブート・ローダーから呼ばれる我々の`_start`エントリ・ポイントは、未だにからです。
それあから何らかをスクリーンに出力するときです。

### スクリーンへの印字

現段階で、スクリーンにテキストを印刷する最も簡単な方法は、[VGAテキスト・バッファ](https://en.wikipedia.org/wiki/VGA-compatible_text_mode)です。
スクリーンに表示されるコンテンツを含んだVGAハードウェアにマップされた特別なメモリ領域です。
それは、通常それぞれ80文字のセルを含んだ25行で構成されています。
それぞれの文字セルは任意の全景と背景色を持つアスキー文字を表示します。
スクリーンへの出力は、このとおりです。

![スクリーンへの出力](https://upload.wikimedia.org/wikipedia/commons/f/f8/Codepage-437.png)

次の投稿で正確なVGAバッファの正確なレイアウトを議論する予定なので、そのために最初の小さなドライバを記述します。
"Hello World!"を印字するために、バッファが`0xb8000`番地に配置され、そしてそれぞれの文字セルがASCIIバイトとカラー・バイトで構成されていることを知る必要があります。

その実装はこの通りです。

```rust
static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}
```

まず、整数`0xb8000`を[生ポインタ](https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer)にキャストします。
そして、[static](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime)な`HELLO`の[バイト文字列](https://doc.rust-lang.org/reference/tokens.html#byte-string-literals)を[反復処理](https://doc.rust-lang.org/stable/book/ch13-02-iterators.html)します。
実行中の変数`i`を付加的に得るために[`enumerate`](https://doc.rust-lang.org/core/iter/trait.Iterator.html#method.enumerate)メソッドを使用します。
ループ本体の中で、文字列バイトと色に対応するバイト（`0xb`はライト・シアン）を記述するために[`offset`](https://doc.rust-lang.org/std/primitive.pointer.html#method.offset)メソッドを使用します。

すべてのメモリの書き込みに[`unsafe`](https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html)ブロックがあることに注意してください。
その理由は、Rustコンパイラは我々が作成した生ポインタが有効であるかを証明することができないからです。
それらはどこかを指し示し、データの破壊を導くかもしれません。
それらを`unsafe`ブロックに入れることにより、基本的に我々は操作が妥当であることを絶対的に確信していることを、コンパイラに伝えています。
`unsafe`ブロックがRustの安全性チェックを無効にしないことに注意してください。
それは単に[追加的な5つのこと](https://doc.rust-lang.org/stable/book/ch19-01-unsafe-rust.html#unsafe-superpowers)をすることを許可するだけです。

私は、**これはRustでやりたかった方法ではない**ことを強調したいと考えています。
unsafeブロック内で生ポインタを操作すると、とても簡単に混乱します。
例えば、我々が注意深くない場合、我々は容易にバッファの後ろを超えて書き込むことができます。

よって、我々は、可能な限り`unsafe`の仕様を最小限にしたいと考えています。
Rustは安全の抽象化を作成することにより、これをする能力を与えてくれます。
例えば、我々はすべての危険性を閉じ込め、それが外部から不正な行為をすることができないように、VGAバッファ型を作成できます。
このようにして、最小の量の`unsafe`コードのみを使用して、[メモリ安全性](https://en.wikipedia.org/wiki/Memory_safety)に違反しないことを確信することがｄきます。
我々は、次の投稿で安全なVGAバッファの抽象化を作成する予定です。

## カーネルを実行する

現在、我々は、目に見えることをする実行形式を持っているので、それを実行する時間です。
最初に、我々はコンパイルしたカーネルをブートローダーとリンクして、起動可能なディスク・イメージに変換する必要があります。
そして、我々は[QEMU]()仮想マシンでディスク・イメージを実行でき、またUSBsティックを使用して実際のハードウェアでそれを起動できる。

### ブートイメージの作成

我々のコンパイルしたカーネルを起動可能なディスク・イメージに変換するために、カーネルをブートローダーとリンクする必要があります。
[ブートに関するセクション](https://os.phil-opp.com/minimal-rust-kernel/#the-boot-process)で学んだように、ブートローダはCPUを初期化して我々のカーネルを呼び出す責任があります。

ブートローダーを記述する代わりに、ブートローダー自身のプロジェクトがあるため、[`bootloader`](https://crates.io/crates/bootloader)クレートを使用します。
このクレートはC、Rustおウヨにインライン・アッセンブリへの依存無しで基本的なBIOSのブートローダーを実装しています。
我々のカーネルを起動するためにそれを使用するため、それを依存関係に追加する必要があります。

``toml
# in Cargo.toml
[dependencies]
bootloader = "0.9.23"
```

依存関係としてブートローダーを追加することは、実際に起動可能なディスクを作成するために十分ではありません。
この問題は、コンパイルの後で我々のカーネルとブートローダーをリンクする必要がありますが、cargoは[ビルドした後のスクリプトの実行](https://github.com/rust-lang/cargo/issues/545)をサポートしていません。

この問題を解決するために、我々は最初にカーネルとブートローダーをコンパイルして、次に起動可能なディスク・イメージを作成するために、それらをリンクする`bootimage`と命名したツールを作成しました。
そのツールをインストールするために、ターミナルで次のコマンドを実行してください。

```bash
cargo install bootimage
```

`bootimage`とブートローダーをビルドするために、`llvm-tools-preview`rustupコンポーネントがインストールされている必要があります。
`rustup component add llvm-tools-preview`を実行することでそれができます。


`bootimage`をインストールして`llvm-tools-preview`コンポーネントを追加した後で、次を実行することにより、起動可能なディスク・イメージを作成できます。

```bash
cargo bootimage
```

そのツールが`cargo build`を使用して我々のカーネルを再コンパイルすることを確認できるので、行った変更が自動的に反映されます。
その後、それはブートローダーをコンパイルしますが、時間がかかるかもしれません。
すべてのクレートの依存関係のように、それは1度のビルドでそれらをキャッシュするため、それに続くビルドはより早くなります。
最終に、`bootimage`はブートローダーとカーネルを起動可能なディスクに結合します。

そのコマンドを実行した後、`target/x86_64-blog_os/debug`ディレクトリ内に、`bootimage-blog_os.bin`と名前を付けられた起動可能なディスク・イメージを確認できます。
仮想マシンで起動するか、USBドライブドライブにコピーして実際のハードウェアで起動できます。
（これはCDイメージ出ないため、異なるフォーマットを持っており、それをCDに書き込んでも動作しないことに注意してください。）

#### どのように動作させるのか？

`bootimage`ツールは背後で以下のステップを実行します。

* 我々のカーネルを[ELF](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format)ファイルにコンパイルします。
* 単独な実行形式としてブートローダーの依存関係をコンパイルします。
* カーネルのELFファイルのバイト列とブートローダーをリンクします。

起動したとき、ブートローダーは追加されたELFファイルを読み込んで解析します。
次に、それはプログラムの断片をページ・テーブル内の仮想アドレスにマップして、.bssセクションをゼロにして、スタックを準備します。
最後に、それはエントリ・ポイントのアドレス（`_start`関数）を読んで、それにジャンプします。

## QEMUで起動する

現在、仮想マシン内でディスク・イメージを起動できます。
[QEMU]()ないでそれを起動するために、次のコマンドを実行してください。

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-blog_os/debug/bootimage-blog_os.bin
```

これは、これに似た分離したウィンドウを開きます。

![Hello World in QEMU](https://os.phil-opp.com/minimal-rust-kernel/qemu.png)

"Hello World"がスクリーンに表示されたことを確認できます。

## 実マシン

それをUSBスティックに書き込み、そして実マシンで起動することもできますが、そのデバイスのすべてを上書きするため、正しいデバイス名を選択するように**注意してください**。

```bash
dd if=target/x86_64-blog_os/debug/bootimage-blog_os.bin of=/dev/sdX && sync
```

そこの`sdX`はUSBスティックにのデバイス名です。

USBスティックにイメージを書き込んだ後で、実マシンをそのデバイスから起動することで、それを実行できます。
USBスティックから起動するために、おそらく特別なブート・メニューまたはBIOS設定内のブート順を変更する必要があると考えられます。
`bootloader`クレートはUEFIをサポートしていないため、それは現在UEFI機で動作しないことに注意してください。

### `cargo run`の仕様

QEMU内でカーネルの実行を容易にするために、cargoの`runner`設定キーを設定できます。

```toml
# in .cargo/config.toml
[target.'cfg(target_os = "none")']
runner = "bootimage runner"
```

`target.'cfg(target_os = "none")'`テーブルは、すべてのターゲットのターゲット設定ファイルの`"os"`フィールドを`"none"`にせっています。
これは、`x86_64-blog_os.json`ターゲットを含みます。
`runner`キーは`cargo run`を実行するコマンドを指定します。
そのコマンドは最初の引数として渡した実行形式ファイルのビルドが成功した後で、実行されます。
詳細は[cargoドキュメント](https://doc.rust-lang.org/cargo/reference/config.html)を参照してください。

`bootimage runner`コマンドは、`runner`が実行可能として使用できるように特別に設計されています。
それは与えられた実行形式をプロジェクトのブートローダーの依存関係にリンクした後、次にQEMUを立ち上げます。
詳細と利用可能な設定オプションは[`bootimage`の説明](https://github.com/rust-osdev/bootimage)を参照してください。

現在、`cargo run`を使用することで、カーネルをコンパイルしてQEMU内で起動することができる。

## 次はなんですか？

次の投稿では、より詳細にVGAテキスト・バッファを探求して、それのための安全なインターフェースを記述します。
また、`println`マクロのサポートを追加します。
