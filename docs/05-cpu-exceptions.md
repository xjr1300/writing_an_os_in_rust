# CPU例外

CPU例外は、例えば不正なメモリ・アドレスにアクセスしたとき、また0で除算したときなど、さまざまな誤った状況下で発生します。
それらに反応するために、処理関数を提供する*割り込み記述子テーブル*を準備する必要があります。
この投稿の最後で、我々のカーネルは[停止例外(`breakpoint exception`)](https://wiki.osdev.org/Exceptions#Breakpoint)を受け取り、後で普通に実行を再開できるようにする予定です。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/cpu-exceptions/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-05`](https://github.com/phil-opp/blog_os/tree/post-05)ブランチで見つけることができます。

## 概要

例外は、現在の命令に問題があることを示します。
例えば、もし現在の命令が0で除算することを試行した場合、CPUは例外を発行します。
例外が発行されたとき、CPUは現在の仕事に割り込み、すぐに例外の種類によった特別な例外処理関数を呼び出します。

x86に置いて、約20種類のさまざまなCPU例外があります。
もっと重要なのは次です。

* **ページ・フォルト:** ページ・フォルトは不正なメモリ・アクセスで発生します。
  例えば、もし現在の命令がマップされていないページから読み込むことを試行したり、読み込み専用ページに書き込むことを試行したりすることです。
* **不正なオペコード:** この例外は、例えば、新しい[SSE命令](https://en.wikipedia.org/wiki/Streaming_SIMD_Extensions)をサポートしていない古いCPUで使用することを試みた場合など、現在の命令が不正なときに発生します。
* **一般保護違反:** これは広い範囲では発生する例外です。
  ユーザー・レベルコード内で特権命令の実行を試みたときや、設定レジスタ内の予約されたフィールドに書き込みしたときなど、さまざまな種類のアクセス違反で発生します。
* **ダブル・フォルト:** 例外が発生した時、CPUは対応するハンドラ関数を呼び出すことを試みます。
  もし、例外ハンドラを呼び出しているときに他の例外が発生した場合、CPUはダブル・フォルト例外を起こします。
  この例外は例外のハンドラ関数が登録されていない場合にも発生します。
* **トリプル・フォルト:** もし、CPUがダブル・フォルト処理関数の呼び出しを試みている間に例外が発生した場合、CPUは致命的なトリプル・フォルトを発行します。
  我々はトリプル・フォルトを受け取り処理することができません。
  ほとんどのプロセッサーはそれらをリセットするか、オペレーティング・システムの再起動することで、反応します。

例外の完全なリストは、[OSDevウィキ](https://wiki.osdev.org/Exceptions)を確認してください。

### 割り込み記述子テーブル

例外を受け取り処理するために、*割り込み記述子テーブル（IDT）*を準備する必要があります。
このテーブル内には、それぞれのCPU例外に対する特別は処理関数を指定できます。
ハードウェアは直接このテーブルを使用するため、事前に定義された書式に従う必要があります。
それぞれのエントリは次の16バイト構造を持つ必要があります。

| 型  | 名前                 | 説明                                                                                                      |
| --- | -------------------- | --------------------------------------------------------------------------------------------------------- |
| u16 | 関数ポインタ [0:15]  | 処理関数を指すポインタの下位ビット                                                                        |
| u16 | GDTセレクタ          | [グローバル記述子テーブル](https://en.wikipedia.org/wiki/Global_Descriptor_Table)内のコード断片のセレクタ |
| u16 | オプション           | （下を参照してください）                                                                                  |
| u16 | 関数ポインタ [16:31] | 処理関数を指すポインタの真ん中のビット                                                                    |
| u32 | 関数ポインタ [32:63] | 処理関数を指すポインタの残りのビット                                                                      |
| u32 | 予約                 | ―                                                                                                         |

オプション・フィールドは以下の書式に従っています。

| ビット | 名前                                     | 説明                                                                                                                  |
| ------ | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| 0-2    | 割り込みスタック・テーブル・インデックス | 0: スタックを切り替えない、1-7: このハンドラが呼ばれたとき、割り込みスタック・テーブルないのn番目のスタックを切り替え |
| 3-7    | 予約                                     | ー                                                                                                                    |
| 8      | 0: 割り込みゲート、1: トラップ・ゲート   | もしこのビットが0の場合、このハンドラが呼ばれたとき割り込みを無視する。                                               |
| 9-11   | 1でなくてはならない                      | ー                                                                                                                    |
| 12     | 0でなくてはならない                      | ー                                                                                                                    |
| 13-14  | 記述特権レベル（DPL）                    | このハンドラを呼び出すために要求される最小の特権レベル                                                                |
| 15     | Present                                  | ー                                                                                                                    |

それぞれの例外は事前に定義されたIDTインデックスを持っています。
例えば、不正オペコード例外はテーブル・インデックス6を持ち、ページ・フォルト例外はテーブル・インデックス14を持ちます。
従って、ハードウェアは、それぞれの例外ごとに、自動的に対応するIDTエントリをロードすることができます。
OSDevウィキないの[例外テーブル](https://wiki.osdev.org/Exceptions)はすべての例外のIDTインデックスを、"Vector nr."列の中で紹介しています。

例外が発生したとき、大体CPUは次をします。

1. スタックにれいれいポインタと[RFLAGS](https://en.wikipedia.org/wiki/FLAGS_register)レジスタを含む任意のレジスタを登録します。
   （このポストにおいて後でこれらの値を使用する予定です。）
2. 割り込み記述子テーブル（IDT）から対応するエントリを読み込みます。
   例えば、ページ・フォルトが発生したとき、CPUは14番目のエントリを読みます。
3. エントリが存在するか確認して、もし存在しない場合は、ダブル・フォルトを起こします。
4. もしエントリが割り込みゲート（ビット40が設定されていない）の場合、ハードウェア割り込みを無効にします。
5. 特定の[GDT]()セレクタをロードしてCS（コード・セグメント）に入れます。
6. 特定の処理関数にジャンプします。

今のところ、ステップ4と5について心配しないでください。
これからの投稿において、グローバル記述子テーブルとハードウェア例外について学ぶ予定です。

## IDT型

我々独自のIDT型を作成する代わりに、`x86_64`クレートの[`InterruptDescriptorTable`構造体](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html)を使用します。それはこのように見えます。

```rust
#[repr(C)]
pub struct InterruptDescriptorTable {
    pub divide_by_zero: Entry<HandlerFunc>,
    pub debug: Entry<HandlerFunc>,
    pub non_maskable_interrupt: Entry<HandlerFunc>,
    pub breakpoint: Entry<HandlerFunc>,
    pub overflow: Entry<HandlerFunc>,
    pub bound_range_exceeded: Entry<HandlerFunc>,
    pub invalid_opcode: Entry<HandlerFunc>,
    pub device_not_available: Entry<HandlerFunc>,
    pub double_fault: Entry<HandlerFuncWithErrCode>,
    pub invalid_tss: Entry<HandlerFuncWithErrCode>,
    pub segment_not_present: Entry<HandlerFuncWithErrCode>,
    pub stack_segment_fault: Entry<HandlerFuncWithErrCode>,
    pub general_protection_fault: Entry<HandlerFuncWithErrCode>,
    pub page_fault: Entry<PageFaultHandlerFunc>,
    pub x87_floating_point: Entry<HandlerFunc>,
    pub alignment_check: Entry<HandlerFuncWithErrCode>,
    pub machine_check: Entry<HandlerFunc>,
    pub simd_floating_point: Entry<HandlerFunc>,
    pub virtualization: Entry<HandlerFunc>,
    pub security_exception: Entry<HandlerFuncWithErrCode>,
    // some fields omitted
}
```

フィールドは[`idt::Entry<F>`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.Entry.html)を持ち、それはIDTエントリのフィールドを表現する構造となっています（上記のテーブルを確認してください。）。
型パラメーター`F`は予期する処理関数の型を定義します。
いくつかのエントリが[`HandlerFunc`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFunc.html)を要求して、いくつかのエントリは[`HandlerFuncWithErrCode`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFuncWithErrCode.html)を要求していることを確認できます。

最初に`HandlerFunc`型を見ましょう。

```rust
type HandlerFunc = extern "x86-interrupt" fn (_: InterruptStackFrame);
```

それは`extern "x86-interrupt" fn`型の[型エイリアス](https://doc.rust-lang.org/book/ch19-04-advanced-types.html#creating-type-synonyms-with-type-aliases)です。
`extern`キーワードは[外部呼び出し規約]()で関数を定義して、Cのコードとの会話によく使用されます（`extern "C" fn`）。
しかし、`x86-interrupt`呼び出し規約とは何でしょうか？

## 割り込み呼び出し規約

例外は関数呼び出しととても良く似ています。
CPUは呼び出された関数の最初の命令にジャンプして、それを実行します。
その後、CPUは戻りアドレスにジャンプして、親関数の実行を継続します。

しかし、例外と関数呼び出しには主要な違いがあります。
関数呼び出しはコンパイラが挿入した`call`命令によって自発的に呼び出されますが、例外は*任意の*命令で発生するかもしれません。
この違いの結果を理解するために、より詳細に関数呼び出しを調べる必要があります。

[呼び出し規約](https://en.wikipedia.org/wiki/Calling_convention)は関数呼び出しの詳細を規定します。
例えば、それらは関数のパラメーターがどこに置かれ（例えば、レジスタまたはスタック）、そしてどのような結果が返却されるのかなどを規定します。
x86_64のLinuxに置いて、以下のルールがC関数に適用されます（[System V ABI](https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf)で規定されています）。

> `ABI`: `Application Binary Interface`

* 最初の6個の整数引数は、レジスタ`rdi`、`rsi`、`rdx`、`rcx`、`r8`、`r9`に渡されます。
* 追加の引数はスタックで渡されます。
* 結果は`rax`と`rdx`に戻されます。

RustはCのABIに従っていない（実際、[RustのABIは未だ無い](https://github.com/rust-lang/rfcs/issues/600)）ので、これらのルールは`extern "C" fn`として宣言された関数のみに適用されます。

### プリザーブド・レジスタとスクラッチ・レジスタ

呼び出し規約はレジスタをプリザーブド・レジスタとスクラッチ・レジスタの2つに分離します。

*プリザーブド*・レジスタの値は関数呼び出し間で変更されずに残っている必要があります。
従って、呼び出された関数（"callee"）は、戻る前にオリジナルの値をリストアする場合にのみ、これらのレジスタを上書きできます。
よって、これらのレジスタは"*callee-saved*"と呼ばれています。
一般的なパターンは、関数の開始でこれらのレジスタをスタックに保存して、戻る前に単にリストアします。

対称的に、呼び出された関数は、制限無しでスクラッチ・レジスタの上書きを許可します。
もし、呼び出し側関数呼び出しの間、スクラッチ・レジスタの値を残したいと望む場合、関数を呼び出す前に（例えば、それをスタックに蓄積することによって）それをバックアップしてリストアする必要があります。
よって、スクラッチ・レジスタは*呼び出し側が保存します*。

x86_64において、Cの呼び出し規約は以下のプリザーブド・レジスタとスクラッチ・レジスタを規定します。

| プリザーブド・レジスタ                          | スクラッチ・レジスタ                                        |
| ----------------------------------------------- | ----------------------------------------------------------- |
| `rbp`, `rbx`, `rsp`, `r12`, `r13`, `r14`, `r15` | `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r8`, `r9`, `r10`, `r11` |
| 呼び出された側が保存                            | 呼び出し側が保存                                            |

コンパイラはこれらのルールを知っているので、それに応じてコードを生成します。
例えば、ほとんどの関数は`push rbp`で開始して、それは`rbp`をスタックにバックアップします（それは呼び出された側が保存するレジスタであるため）。

### すべてのレジスタを保存する

関数呼び出しとは対称的に、例外は*任意の*命令で発生する可能性があります。
大半のケースに置いて、生成されたコードが例外を引き起こすかを、コンパイル時に知ることができません。
例えば、コンパイラは、もし命令がスタック・オーバーフローまたはページ・フォルトを引き起こすかを、知ることができません。

いつ例外が発生するか知ることができないため、前もって任意のレジスタをバックアップできません。
これは、例外処理のために呼び出し側が保存するレジスタに依存する呼び出し規約を使用することができないことを意味します。
`x86-interrupt`呼び出し規約は、そのような呼び出し規約であるため、関数が戻った時に、それはすべてのレジスタの値をそれらのオリジナルな値でリストアすることを保証します。

これは、関数に入ったときに、すべてのレジスタがスタックに保存されることを意味していないことに注意してください。
代わりに、コンパイラは、関数によって上書きされたレジスタのみをバックアップします。
このようにして、ほんの少しのレジスタしか使わない短い関数に対して、とても効率的なコードが生成されるようになります。

### 割り込みスタック・フレーム

普通の関数呼び出し（`call`命令を使用するような）において、CPUは目的の関数にジャンプする前に、戻りアドレスをプッシュします。
関数の戻り時には（`ret`命令を使用します）、CPUは戻りアドレスをポップして、それにジャンプします。
従って、普通の関数のスタック・フレームは、このように見えます（下に向かって、スタックが伸びていく？）。

![a stack frame of function](https://os.phil-opp.com/cpu-exceptions/function-stack-frame.svg)

しかし、例外と割り込み処理にとって、戻りアドレスをプッシュすることは十分ではないため、割り込み処理はよく、異なるコンテキスト（スタック・ポインタ、CPUフラグなど）内で実行されます。
代わりに、例外が発生したとき、CPUは次の手順を実行します。

0. **古いスタック・ポインタの保存:** CPUはスタックポインタ（`rsp`）と、スタック・セグメント（`ss`）レジスタの値を読み込み、内部のバッファにそれらを記憶します。
1. **スタック・ポインタの調整:** 割り込みは任意の命令で発生する可能性があるため、スタック・ポインタは任意の値を持っている可能性がある。
   しかし、いくつかのCPU命令（例えば、任意のSSE命令）は、スタック・ポインタが16バイト境界で調整されていることを要求するので、CPUは割り込みの直後でそのような調整を実行する。
2. **スタックの切り替え** （いくつかの場合）: 例えば、CPU例外がユーザー・モード・プログラムで発生した場合など、CPU特権レベルが変更されたとき、スタックの切り替えが発生します。
   それは、いわゆる*割り込みスタック・テーブル*（次の投稿で説明します）を使用して、特定の割り込みのためにスタックの切り替えを設定することができます。
3. **古いスタックポインタのプッシュ:** CPUは手順0の`rsp`と`ss`値をスタックにプッシュします。
   これは、割り込み処理から戻るときに、オリジナルのスタック・ポインタをリストアできるようにします。
4. **`RFLAGS`レジスタのプッシュと更新:** [`RFLAGS`](https://en.wikipedia.org/wiki/FLAGS_register)レジスタはいろいろな制御と状態ビットを含んでいます。
   割り込みエントリにおいて、CPUはいくつかのビットを変更して、古い値をプッシュします。
5. **命令ポインタのプッシュ:** 割り込み処理関数にジャンプする前に、CPUは命令ポインタ（`rip`）とコード・セグメント（`cs`）をプッシュします。
   これは普通の関数呼び出しの戻りアドレスのプッシュと同じです。
6. **エラー・コードのプッシュ**（いくつかの例外のため）: ページ・フォルトのような、いくつかの特定の例外のために、CPUはエラー・コードをプッシュして、それは例外の原因を説明します。
7. **割り込み処理の呼び出し:** CPUは、IDT内の対応するフィールドから、アドレスと割り込み処理関数のセグメント記述子を読み込みます。
   次に、それは、値を`rip`と`cs`レジスタにロードすることで、この処理を呼び出します。

よって、*割り込みスタック・フレーム*はこのように見えます。（下に向かって、スタックが伸びていく？）。

![interrupt stack frame](https://os.phil-opp.com/cpu-exceptions/exception-stack-frame.svg)

`x86_64`クレートにおいて、割り込みスタック・フレームは、[`InterruptStackFrame`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptStackFrame.html)構造体で表現されています。
それは、`&mut`で割り込み処理を渡されて、例外の原因に関する追加情報を取得するために使用されます。
エラー・コードをプッシュする例外は小数に限られるため、その構造体はエラー・コード・フィールドを含んでいません。
これらの例外は、付加的な`error_code`引数を持つ、別の[`HandlerFuncWithErrCode`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/type.HandlerFuncWithErrCode.html)関数型を使用します。

### 場面の背後

`x86-interrupt`呼び出し規約は、ほとんどすべての例外処理プロセスの複雑な詳細を隠す、強力な抽象化です。
しかし、ときどき、それは、カーテンの背後で何が起こったかを知ることに役立ちます。
これは`x86-interrupt`呼び出し規約が処理する短い概要です。

* **引数の受け取り:** ほとんどの呼び出し規約は、引数がレジスタに渡されることを予期しています。
  これは例外処理ではできないので、任意のレジスタの値をスタックにバックアップする前に、上書きする必要があります。
  代わりに、`x86-interrupt`呼び出し規約は、引数が特定のオフセットでスタックに既に存在することを認識しています。
* **`iretq`を使用した戻り**: 割り込みスタック・フレームは、普通の関数呼び出しのスタック・フレームとは完全に異なるため、普通の`ret`命令を通じて処理関数から戻ることができません。
  よって、代わりに、`iretq`命令が使用されなくてはなりません。
* **エラー・コードの処理:** いくつかの例外によってプッシュされたエラー・コードは、ことをよりさらに複雑にします。
  それはスタックの調整を変更して（次のポイントをみて）、戻る前にスタックから取り出す必要があります。
  `x86-interrupt`呼び出し規約は、その複雑さをすべて処理します。
  しかし、それは、どの例外でどの処理関数が使用されるかを理解していないため、それは関数の引数の数からその情報を推測する必要があります。
  それはプログラマが未だそれぞれの例外ごとに正しい関数の型を使用する責任があることを意味します。
  幸運にも、`InterruptDescriptorTable`型は、正確な関数の方が使用されることを保証する`x86_64`クレートによって定義されています。
* **スタックの調整:** いくつかの命令（特にSSE命令）は16バイトのスタックの調整を要求します。
  CPUは、例外発生したときはいつでも、この調整を保証しますが、一部の例外については、後でエラー・コードをプッシュするときに再度破棄します。
  `x86-interrupt`呼び出し規約は、このケースのスタックの再調整することで、これを処理します。

詳細に興味がある場合は、この投稿の[最後](https://os.phil-opp.com/cpu-exceptions/#too-much-magic)にリンクされている[ネイキッド関数](https://github.com/rust-lang/rfcs/blob/master/text/1201-naked-fns.md)を使用した例外処理を説明する一連の投稿もあります。

## 実装

現時点で、原理を理解したため、我々のカーネルでCPU例外を処理する時間です。
`src/interrupts.rs`に、新しい割り込みモジュールを作成することから始めて、最初に新しい`InterruptDescriptorTable`を作成する`init_idt`関数を作成します。

```rust
// in src/lib.rs

pub mod interrupts;

// in src/interrupts.rs

use x86_64::structures::idt::InterruptDescriptorTable;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
}
```

現時点で、処理関数を追加することができます。
[ブレイクポイント例外](https://wiki.osdev.org/Exceptions#Breakpoint)に対応する処理を追加することから始めます。
ブレイクポイント例外は例外処理をテストするために完璧な例外です。
ブレイクポイント命令`int3`が実行されたとき、それがプログラムを一時的に停止することが唯一の目的です。

一般に、ブレイクポイント命令はデバッガで使用されます。
ユーザーがブレイクポイントを設定した時、デバッガは対応する命令を`int3`命令で上書きすることで、デバッガがその行に到達したときにCPUはブレイクポイント例外を投げます。
ユーザーがプログラムを継続したいとき、デバッガは再度`int3`命令をオリジナルの命令に置き換えて、プログラムを継続します。
詳細は、一連の[**"どのようにデバッガが動作するか"**](https://eli.thegreenplace.net/2011/01/27/how-debuggers-work-part-2-breakpoints)を参照してください。

我々の場合のために、任意の命令を上書きする必要はありません。
代わりに、ブレイクポイント命令が実行されたときと、次にプログラムが継続されたときに、メッセージを単に出力したいと思います。
よって、単純な`breakpoint_handler`関数を作成して、それに我々のIDTを追加します。

```rust
// in src/interrupts.rs

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::println;

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}
```

我々の処理は単にメッセージを出力して、割り込みスタック・フレームを整形表示します。

コンパイルを試みた時、次のエラーが発生します。

```
error[E0658]: x86-interrupt ABI is experimental and subject to change (see issue #40180)
  --> src/main.rs:53:1
   |
53 | / extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
54 | |     println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
55 | | }
   | |_^
   |
   = help: add #![feature(abi_x86_interrupt)] to the crate attributes to enable
```

このエラーは、`x86-interrupt`呼び出し規約がいまだにアンセーフであることが理由です。
どうにかそれを使用するために、我々の`lib.rs`の最上部に`#![feature(abi_x86_interrupt)]`を追加することで、それを明示的に有効にする必要があります。

```rust
// in src/lib.rs

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![feature(abi_x86_interrupt)]      // NEW
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
```

### IDTのロード

CPUが我々の新しい割り込み記述子ターブルを使用するために、[`lidt`](https://www.felixcloutier.com/x86/lgdt:lidt)命令を使用して、それをロードする必要があります。
`x86_64`クレートの`InterruptDescriptorTable`構造体は、そのために[`load`]()メソッドを提供しています。
それを利用してみましょう。

```rust
// in src/interrupts.rs

pub fn init_idt() {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.load();
}
```

現段階でそれをコンパイルを試みたとき、以下のエラーが発生します。

```
error: `idt` does not live long enough
  --> src/interrupts/mod.rs:43:5
   |
43 |     idt.load();
   |     ^^^ does not live long enough
44 | }
   | - borrowed value only lives until here
   |
   = note: borrowed value must be valid for the static lifetime...
```

`load`メソッドは`&'static self`を予期しているため、参照がプログラムの実行時間で完全に有効である必要があります。
その理由は、他のIDTをロードするまで、すべての割り込みについてこのテーブルをアクセスするからです。
よって、`'static`よりも短いライフタイムの使用は、解放後使用のバグを導く可能性があります。

実際、これは正確にここで発生していることです。
我々の`idt`はスタック上に作成されるため、それは`init`関数内でのみ有効です。
その後、スタック・メモリが他の関数に再利用されるため、CPUはIDTとしてランダムなスタック・メモリを解釈します。
幸運にも、`InterruptDescriptorTable::load`メソッドは、その関数定義でこのライフタイム要件を符号化するため、Rustコンパイラはコンパイル時にこの発生する可能性のあるバグを回避することができます。

この問題を解決するために、`idt`が`'static`ライフタイムを持つ場所に格納する必要があります。
これを達成するために、[`Box`](https://doc.rust-lang.org/std/boxed/struct.Box.html)を使用して我々のIDTをヒープに確保して、後でそれを`'static`参照に変換しますが、OSカーネルを記述しているため、ヒープを持っていません（まだ）。

代替手段として、`static`でIDTを蓄積することを試します。

```rust
static IDT: InterruptDescriptorTable = InterruptDescriptorTable::new();

pub fn init_idt() {
   IDT.breakpoint.set_handler_fn(breakpoint_handler);
   IDT.loac();
}
```

しかし、これには問題があります。
静的変数は不変であるため、我々の`init_idt`関数からブレイクポイント・エントリを変更できません。
[`static mut`](https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable)を使用して、これを解決できるかもしれません。

```rust
satic mut IDT: InterruptDescriptorTable = InterruputDescriptorTable::new();

pub fn init_idt() {
   IDT.breakpoint.set_handler_fn(breakpoint_handler);
   IDT.load();
}
```

この変形はエラーなしでコンパイルしますが、慣用的とは言えません。
`static mut`はデータ競合をとても生み出しやすいので、それぞれのアクセスで[`unsafe`ブロック](https://doc.rust-lang.org/1.30.0/book/second-edition/ch19-01-unsafe-rust.html#unsafe-superpowers)が必要になります。

#### 遅延静的変数が救う

幸運にも、`lazy_static`マクロが存在します。
コンパイル時に`static`と評価される代わりに、そのマクロは最初に`静的変数`が参照されたときに、初期化を実行します。
従って、初期化ブロック内にほとんどすべてを実行して、ランタイムの値を読み込むことができます。

[VGAテキスト・バッファのための抽象化を作成した](https://os.phil-opp.com/vga-text-mode/#lazy-statics)ときに、すでに`lazy_static`クレートをインポートしています。
よって、我々の静的なIDTを作成するために、`lazy_static!`マクロを直接利用できます。

```rust
// in src/interrupts.rs

use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}
```

この解決方法が`unsafe`ブロックを要求しないことに注意してください。
`lazy_static!`マクロは舞台裏で`unsafe`を使用しますが、それは安全なインターフェースに抽象化されています。

### 実行する

我々のカーネル内で例外を機能させるための最後の手順は、我々の`main.rs`から`init_idt`関数を呼び出すことです。
それを直接呼び出す代わりに、我々の`lib.rs`に一般的な`init`関数を導入します。

```rust
// in src/lib.rs

pub fn init() {
    interrupts::init_idt();
}
```

この関数で、現在、異なる我々の`main.rs`と`lib.rs`内の`_start`関数と統合テスト間で共有できる、初期化ルーチンのための中心地点を持ちました。

現時点で、`init`を呼び出し、その後でブレイクポイント例外を発行するために、我々の`main.rs`の`_start`関数を更新できます。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init(); // new

    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3(); // new

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    loop {}
}
```

現段階で、それをQEMU内で実行したとき（`cargo run`を使用して）、次を確認できます。

![invoke a breakpoint exception](https://os.phil-opp.com/cpu-exceptions/qemu-breakpoint-exception.png)

機能しました！
CPUは我々のブレイクポイント処理を成功裏に呼び出し、それはメッセージを出力して、その後`_start`関数に戻り、それは`It did not crash!`メッセージを表示しました。

割り込みスタック・フレームは、例外が発生したときの命令と、その時のスタック・ポインタを伝えます。
この情報は、予期しない例外をデバッグするときに、とても役に立ちます。

### テストを追加する

上記の実装が機能することを継続することを保証するテストを作成しましょう。
最初に`init`を呼び出すために`_start`関数も更新します。

```rust
// in src/lib.rs

/// `cargo test`のエントリ・ポイント
#[cfg(test)]
#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    init(); // new
    test_main();

    loop {}
}
```

この`_start`関数は、`cargo test --lib`を実行したときに使用されるため、Rustは完全に`main.rs`とは独立して`lib.rs`をテストすることを思い出してください。
テストを実行する前にIDTを準備するために、ここで`init`を呼び出す必要があります。

現時点で、`test_breakpoint_exception`テストを作成できます。

```rust
// in src/lib.rs

#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}
```

テストはブレイクポイント例外を発するために`int3`関数を呼び出します。
後で実行を継続することを確認するために、我々のブレイクポイント処理が正しく機能していることを検証します。

`cargo test`（すべてのテスト）または`cargo test --lib`（`lib.rs`のテストとそのモジュール）を実行することにより、この新しいテストを試せます。
出力で次を確認できるはずです。

```
blog_os::interrupts::test_breakpoint_exception...	[ok]
```

## 魔法を使いすぎでしょうか？

`x86-interrupt`呼び出し規約と[`InterruptDescriptorTable`]()型は、例外処理プロセスを比較的直接的に痛みを少なくしました。
もし、これがあなたやあなたや例外処理のすべての血みどろの詳細を学ぶような人にとって魔法を使用しすぎである場合、あなたを援護しました。
我々の一連の["Handling Exceptions with Naked Functions"](https://os.phil-opp.com/edition-1/extra/naked-exceptions/)は、`x86-interrupt`呼び出し規約無しで例外処理する方法と、その独自のIDTを型を作成する方法を説明しています。
歴史的に、これらは`x86-interrupt`呼び出し規約と`x86_64`クレートが存在する前の主要な例外処理の投稿でした。
これらの投稿が、このブログの[初版](https://os.phil-opp.com/edition-1/)を基にしており、時代遅れになっているかもしれません。

## 次は何ですか？

我々の最初の例外を受け取り、それから戻ることに成功しました！
次のステップは、受け取っていない例外が致命的な[トリプル・フォルト](https://wiki.osdev.org/Triple_Fault)を引き起こし、システム・リセットを招くため、すべての例外を受け取ることを保証することです。
次の投稿は、どのように[ダブル・フォルト](https://wiki.osdev.org/Double_Fault#Double_Fault)を正確に受け取ることにより、これを防ぐ方法を説明します。
