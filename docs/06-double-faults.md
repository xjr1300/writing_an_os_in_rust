# 二重違反

この投稿は、例外処理の呼び出しにCPUが失敗したときに発生する、二重違反例外を詳細に探求します。
この例外を処理することにより、システム・リセットの原因となる致命的な*トリプル・フォルト*を防ぎます。
すべての場面でトリプル・フォルトを防止するために、分離したカーネル・スタックで二重違反を受け取るために*割り込みスタック・テーブル*も準備します。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/double-fault-exceptions/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-06`](https://github.com/phil-opp/blog_os/tree/post-06)ブランチで見つけることができます。

## 二重違反とは何でしょう？

簡単に言えば、二重違反は、CPUが例外処理の呼び出しに失敗したときに発生します。
例えば、ページ・フォルトが発せられたが、[割り込み記述子テーブル](https://os.phil-opp.com/cpu-exceptions/#the-interrupt-descriptor-table)（IDT）にページ・フォルト・ハンドラが登録されていないときに発生します。
従って、例えば、C++の`catch (...)`やJavaやC#における`catch (Exception e)`などの、例外を持ったプログラミング言語におけるcatch-allブロックに似ています。

二重違反は普通の例外のように振る舞います。
それはベクタ番号`8`を持つので、IDT内にそれの普通のハンドラ関数を定義できます。
もし二重違反が処理されない場合、致命的な*トリプル・フォルト*が発生するため、二重違反ハンドラを提供することは本当に重要です。
トリプル・フォルトは受け取ることができず、ほとんどのハードウェアはシステム・リセットで反応します。

### 二重違反を発する

ハンドラ関数を定義していないため、例外を発することにより、二重違反を引き起こしましょう。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    // trigger a page fault
    unsafe {
        *(0xdeadbeef as *mut u64) = 42;
    }

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");
    loop {}
}
```

`unsafe`を使用して不正なアドレス`0xdeadbeef`に書き込みします。
その仮想のアドレスは、ページ・テーブル内の物理アドレスにマップされていないため、ページ・フォルトが発生します。
我々はページ・フォルトハンドラを、我々のIDTに登録していないため、二重違反が発生します。

現時点でカーネルを起動した時、無限のブート・ループに入ったことを確認できます。
ブート・ループの理由は次の通りです。

1. CPUは、ページ・フォルトの原因となる`0xdeadbeef`への書き込みを試みます。
2. CPUはIDT内の対応するエントリを探し、ハンドラ関数が指定されていないことを確認します。
   従って、それはページ・フォルトハンドラを呼び出せないため、二重違反が発生します。
3. CPUは二重違反ハンドラのIDT内のエントリを探しますが、このエントリもハンドラ関数が指定されていません。
   従って、トリプル・フォルトが発生します。
4. トリプル・フォルトは致命的です。
   QEMUは、ほとんど本物の波動ェアのようにそれに反応して、システム・リセットを発行します。

よって、このトリプル・フォルトを防ぐために、ページ・フォルトと二重違反のためのハンドラ関数を提供する必要があります。
すべての場面でトリプル・フォルトを避けるために、処理されない例外の型に対して発行される、二重違反ハンドラから始めます。

## 二重違反ハンドラ

二重違反はエラーコードを持つ普通の例外なので、我々のブレイクポイント・ハンドラと同様なハンドラ関数を記述できます。

```rust
// in src/interrupts.rs

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);

        idt
    };
}

// new
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}
```

我々のハンドラは短いエラー・メッセージを出力して、例外スタック・フレームをダンプします。
二重違反ハンドラのエラーコードは常に0であるため、それを出力する理由はありません。
ブレイクポイント・ハンドラと1つ違うことは、二重違反ハンドラは[発散](https://doc.rust-lang.org/stable/rust-by-example/fn/diverging.html)することです。
その理由は、`x86_64`アーキテクチャは、二重違反例外から戻ることを許可していないからです。

現時点で我々のカーネルを起動したとき、二重違反ハンドラが呼び出されたことを確認出来るはずです。

![handle double fault](https://os.phil-opp.com/double-fault-exceptions/qemu-catch-double-fault.png)

それは機能しました！
ここに、この時に発生したことを示します。

1. ページ・フォルトの原因となる`0xdeadbeef`への書き込みを試みます。
2. 前と同様に、CPUはIDTをの対応するエントリを探して、ハンドラ関数が定義されていないことを確認します。
   従って、二重違反が発生します。
3. CPUは、現在存在する、二重違反ハンドラにジャンプします。

トリプル・フォルト（とブート・ループ）は、もはや発生しないため、現在CPUは二重違反ハンドラを呼び出せます。

これはとても直線的です。
なぜ、この話題のために全体の投稿を必要としたのでしょうか？
それは、現在*ほとんど*の二重違反を受け取ることができますが、我々の現在の方法が十分でない場面がいくつかあります。

## 二重違反の原因

特別な場面を見る前に、二重違反が発生する正確な原因を知る必要があります。
上記で、我々はとても漠然とした定義を使用しました。

> 二重違反は、CPUが例外ハンドラの呼び出しに失敗したときに発生する、特別な例外です。

*呼び出しに失敗*とは正確に何を意味するのでしょうか？
ハンドラが存在しないことでしょうか？
ハンドラが[スワップ・アウト](http://pages.cs.wisc.edu/~remzi/OSTEP/vm-beyondphys.pdf)されることでしょうか？
そして、ハンドラそれ自身で例外が発生した場合、何が起こるでしょうか？

例えば、何が起こるでしょうか。

1. ブレイクポイント例外が発生したが、対応するハンドラ関数がスワップ・アウトされていたら？
2. ページ・フォルトが発生したが、ページ・フォルト・ハンドラがスワップ・アウトされていたら？
3. ゼロ除算処理がブレイクポイント例外を発生させたが、ブレイクポイント・ハンドラがスワップ・アウトされていたら？
4. カーネルがそのスタックを溢れさせると、その*保護ページ*はヒットしますか？

幸運にも、AMD64マニュアル（[PDF](https://www.amd.com/system/files/TechDocs/24593.pdf)）は（8.2.9節に）正確な定義を持っています。
それに従うと、「二重違反例外は、前（最初）の例外ハンドラが処理している間に、2番めの例外が発生したときに発生する可能性がある」とのことです。
"可能性がある"は重要です。
とても特別な例外の組だけ二重違反を導きます。
これらの組は次のとおりです。

| First Exception                                                                                                       | Second Exception                                                                                                  |
| --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| [Divide-by-zero],<br>[Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault] | [Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault]                  |
| [Page Fault]                                                                                                          | [Page Fault],<br>[Invalid TSS],<br>[Segment Not Present],<br>[Stack-Segment Fault],<br>[General Protection Fault] |

[Divide-by-zero]: https://wiki.osdev.org/Exceptions#Division_Error
[Invalid TSS]: https://wiki.osdev.org/Exceptions#Invalid_TSS
[Segment Not Present]: https://wiki.osdev.org/Exceptions#Segment_Not_Present
[Stack-Segment Fault]: https://wiki.osdev.org/Exceptions#Stack-Segment_Fault
[General Protection Fault]: https://wiki.osdev.org/Exceptions#General_Protection_Fault
[Page Fault]: https://wiki.osdev.org/Exceptions#Page_Fault

従って、例えば、ページ・フォルトによるゼロ割は良い（ページ・フォルト・ハンドラが呼び出した）ですが、ゼロ割に続く一般保護違反は二重違反を引き起こします。

この表の助けを借りて、上記質問の最初の3つに回答できます。

1. もしブレイクポイント例外が発生して、対応するハンドラ関数がスワップ・アウトされていた場合、*ページ・フォルト*が発生して、*ページ・フォルト・ハンドラ*が呼び出されます。
2. もし、ページ・フォルトが発生して、ページ・フォルト・ハンドラがスワップ・アウトされていた場合、*二重違反*が発生して、*二重違反*ハンドラが呼び出されます。
3. もしゼロ割り処理がブレイクポイント例外を引き起こした場合、CPUはブレイクポイント・ハンドラの呼び出しを試みます。もしブレイクポイント・ハンドラがスワップ・アウトされていた場合、*ページ・フォルト*が発生して、*ページ・フォルト・ハンドラ*が呼び出されます。

実際、IDTにハンドラ関数がない例外は、この枠組みに従います。
例外が発生した時、CPUは対応するIDTエントリの読み込みを試みます。
エントリが0のため、有効なIDTエントリはなく、*一般保護違反*が発生します。
表に従うと、これは二重違反を引き起こします。

### カーネル・スタック・オーバーフロー

4番目の質問に注目しましょう。

> 我々のカーネルがカーネルのスタックをオーバーふr−して、保護ページにヒットした場合、何が起こるでしょうか？

保護ページは、スタック・オーバーフローを検知できるようにするためにスタックの最も下に位置する、特別なメモリ・ページです。
このページは物理的なフレームにマップされないため、それへのアクセスは、黙って他のメモリを破損する代わりに、ページ・フォルトを引き起こします。
ブートローダーは、我々のカーネル・スタックのために保護ページを準備するため、スタック・オーバーフローは*ページ・フォルト*を引き起こします。

ページ・フォルトが発生したとき、CPUはIDT内のページ・フォルト・ハンドラを探して、スタックに[割り込みスタック・フレーム](https://os.phil-opp.com/cpu-exceptions/#the-interrupt-stack-frame)をプッシュします。
しかし、現在のスタック・ポインタは存在しない保護ページを指し示します。
従って、2番目のページ・フォルトが発生して、それはダブル・フォルトを引き起こします（上記の表に従って）。

よって、CPUは*ダブル・フォルト・ハンドラ*を呼び出すことを試みます。
しかし、ダブル・フォルトにおいて、CPUは例外スタック・フレームもスタックにプッシュすることを試みます。
スタック・ポインタは未だ保護ページを指し示しているため、*3番目*のページ・フォルトが発生して、それは*トリプル・フォルト*とシステムの再起動を引き起こします。
よって、この場合、我々の現在のダブル・フォルト・ハンドラは、トリプル・フォルトを防ぐことができません。

それを我々自身で試してみましょう！
無限再帰する関数呼び出しによって、カーネル・スタック・オーバーフローを簡単に引き起こすことができます。

```rust
// in src/main.rs

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");

    blog_os::init();

    fn stack_overflow() {
        stack_overflow(); // for each recursion, the return address is pushed
    }

    // trigger a stuck overflow
    stack_overflow();

    // trigger a page fault
    unsafe {
        *(0xdeadbeef as *mut u64) = 42;
    }

    // as before
    #[cfg(test)]
    test_main();

    println!("It did not crash!");

    loop {}
}
```

QEMU内でこのコードを試した時、再度、システムがブートループに入ることを確認できます。

では、どのようにこの問題を防止出来るでしょうか？
例外スタック・フレームのプッシュを省略することはできないため、CPUそれ自身でそれをします。
よって、ダブル・フォルト例外が発生したとき、スタックが常に有効であることを、どうにかして保証する必要があります。
幸運にも、x86_64アーキテクチャには、この問題に対する解決方法があります。

## スタックの切り替え

x86_64アーキテクチャは、例外が発生したとき、事前に定義されて、知られている良い（known-good）スタックに切り替えできます。
この切り替えはハードウェア・レベルで発生するため、それはCPUが例外スタック・フレームをプッシュする前に実行されます。

この切り替え機構は*割り込みスタック・テーブル*（IST）として実装されています。
ISTは、知られている良い（known-good）スタックを指し示す7つのポインタのテーブルです。

```rust
struct InterruptStackTable {
    stack_pointers:: [Option<StackPointer>; 7],
}
```

それぞれの例外ハンドラのために、対応する[IDTエントリ](https://os.phil-opp.com/cpu-exceptions/#the-interrupt-descriptor-table)内の`stack_pointers`フィールドを通じて、ISTからスタックを選択できます。
例えば、我々のダブル・フォルト・ハンドラはIST内の最初のスタックを使用できます。
CPUは、ダブル・フォルトが発生したときはいつでも自動的にこのスタックに切り替えます。
この切り替えは何かがプッシュされる前に発生することで、トリプル・フォルトを防ぎます。

### ISTとTSS

割り込みスタック・フレーム（IST）は[タスク状態セグメント](https://en.wikipedia.org/wiki/Task_state_segment)（TSS）と呼ばれる古い遺産的な構造の一部です。
TSSは、32ビット・モードのタスクに関するいろいろな情報のかけら（例えば、プロセッサ・レジスタの状態）を保持するために、例えば[ハードウェア・コンテキスト・スイッチ](https://wiki.osdev.org/Context_Switching#Hardware_Context_Switching)のために使用されました。
しかし、64ビット・モードにおいて、ハードウェア・コンテキスト・スイッチはもはやサポートされておらず、またTSSのフォーマットは完全に変更されました。

x86_64において、TSSは何らかタスクの特別な情報を全く保持していません。
代わりに、それは2つのスタック・テーブル（ISTはそれらの1つです）を保持します。
32ビットと64ビットのTSSで共通する唯一のフィールドは、[I/Oポート権限ビットマップ](https://en.wikipedia.org/wiki/Task_state_segment#I.2FO_port_permissions)を指し示すポインタです。

64ビットTSSは次のフォーマットです。

| フィールド                  | 型         |
| --------------------------- | ---------- |
| （予約済み）                | `u32`      |
| 特権スタック・テーブル      | `[u64; 3]` |
| （予約済み）                | `u64`      |
| 割り込みスタック・テーブル  | `[u64; 7]` |
| （予約済み）                | `u64`      |
| （予約済み）                | `u16`      |
| I/Oマップ・ベース・アドレス | `u16`      |

*特権スタック・テーブル*は、特権レベルが変更されたときに、CPUによって使用されます。
例えば、CPUがユーザー・モード（権限レベル3）のときに例外が発生したとき、通常、CPUは例外ハンドラを呼び出す前に、カーネルモード（権限レベル0）に切り替えます。
この場合、CPUは権限スタック・テーブル内の0番目のスタックに切り替えるでしょう（0は目的の権限レベルなので）。
我々は未だにユーザー・モードプログラムを持っていないため、今のところ、このテーブルを無視する予定です。

### TSSの作成

分離したダブル・フォルト・スタックを含む新しいTSSを作成しましょう。
そのために、TSS構造体が必要です。
幸運にも、`x86_64`クレートは、我々が使用できる[`TaskStateSegment`構造体](https://docs.rs/x86_64/0.14.2/x86_64/structures/tss/struct.TaskStateSegment.html)を含んでいます。

TSSは新しい`gdt`モジュールに作成します（後で名前がわかります）。

```rust
use lazy_static::lazy_static;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;

            stack_end
        };

        tss
    };
}
```

Rustの定数評価器は、未だにコンパイル時にこの初期化できるほど強力でないため、`lazy_static`を使用します。
ダブル・フォルト・スタック（任意の他のISTインデックスも機能します）である0番目のエントリを定義しています。
次に、0番目のエントリを指し示す、ダブル・フォルト・スタックの最も上位アドレスを記述しました。
x86において、例えば、上位アドレスから下位アドレスにスタックは下方向に成長するので、最も上位のアドレスを記述しています。

我々はまだメモリ管理を実装していないため、新しいスタックを確保する適切な方法がありません。
代わりに、今のところ、スタック・ストレージとして`static mut`を使用してます。
コンパイラは、可変静的変数がアクセスされたとき、自由な競合を保証できないため、`unsafe`が要求されます。
それが`static mut`であって、不変な`static`でないことが重要な理由は、そうでない場合、ブートローダーはそれを読み込み専用ページとしてマップするためです。
後の投稿で、これを適切なスタック割り当てに置き換える予定で、そのとき`unsafe`はこの場所で必要とされなくなります。

ダブル・フォルト・スタックは、スタック・オーバーフローを保護する保護ページがないことに注意してください。
これは、スタック・オーバーフローが下のスタックを破壊する可能性があるため、我々のダブル・フォルト・ハンドラ内で、スタックに集中した（stack-intensive）ことをするべきではありません。

#### TSSのロード

現時点で、新しいTSSを作成したため、それを使用すべきことをCPUに伝える方法が必要です。
不運にも、TSSがセグメンテーション・システムを使用するため（歴史的な理由で）、これは少し面倒です。
直接テーブルをロードする代わりに、[グローバル記述子テーブル](https://web.archive.org/web/20190217233448/https://www.flingos.co.uk/docs/reference/Global-Descriptor-Table/)（GDT）に新しいセグメンテーション記述子を追加する必要があります。
そして、それぞれのGDTインデックスで[`itr`命令](https://www.felixcloutier.com/x86/ltr)を発行することによって、我々のTSSをロードできます（これが、我々のモジュールを`gdt`と名前をつけた理由です）。

### グローバル記述子テーブル

グローバル記述子テーブル（GDT）は、ページングが標準として認められる前に[メモリ・セグメンテーション](https://en.wikipedia.org/wiki/X86_memory_segmentation)のために使用された遺物です。
しかし、それは、カーネル／ユーザー・モードの構成やTSSのローディングなど、さまざまなことのために、64ビット・モードで必要とされています。

GDTは*セグメントの*プログラムを含んだ構造です。
それは、ページングが標準になる前にプログラムを互いに隔離するために、古いアーキテクチャで使用されています。
セグメンテーションについての詳細は、無料の["Three Easy Pieces"本](http://pages.cs.wisc.edu/~remzi/OSTEP/)の同じ名前の章を参照してください。
もはや、セグメンテーションは64ビット・モードでサポートされていませんが、未だGDTは存在します。
それは、ほとんど2つの方法で使用されています。カーネル空間とユーザー空間の切り替えと、TSS構造のロードです。

#### GDTの作成

我々の`TSS`静的変数のためにセグメントを含む静的な`GDT`を作成しましょう。

```rust
// in src/gdt.rs

use x86_64::structures::tss::TaskStateSegment;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;

            stack_end
        };

        tss
    };
    static ref GDT: GlobalDescriptorTable = {
        let mut gdt = GlobalDescriptorTable::new();
        gdt.add_entry(Descriptor::kernel_code_segment());
        gdt.add_entry(Descriptor::tss_segment(&TSS));

        gdt
    };
}
```

前の通り、再度`lazy_static`を使用します。
コード・セグメントとTSSセグメントと一緒に、新しいGDTを作成しました。

#### GDTのローディング

GDTをローディングするために、我々の`init`関数から呼び出す新しい`gdt::init`関数を作成します。

```rust
// in src/gdt.rs

pub fn init() {
    GDT.load();
}

// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
}
```

現時点で、我々のGDTがロードされます（`_start`関数が`init`を呼び出すため）が、スタック・オーバーフローでブート・ループします。

### 最後の手順

問題は、いまだにセグメントとTSSレジスタが古いGDTからきた値を含んでいるため、まだGDTセグメントが有効にならないことです。
ダブル・フォルトIDTエントリを新しいスタックで使用するため、ダブル・フォルトIDTエントリを修正する必要もあります。

まとめとして、次のことをする必要があります。

1. **コード・セグメント・レジスタの再ロード:** 我々のGDTを変更したため、コード・セグメント・セレクタである`cs`を再ロードする必要があります。
   現在、古いセグメント・セレクタが異なるGDT記述子（例えば、TSS記述子）を指し示す可能性があるため、これは必須です。
2. **TSSのロード:** TSSセレクタを含むGDTをロードしましたが、CPUがそのTSSを使用すべきであることをCPUに伝える必要があります。
3. **IDTエントリの更新:** 可能な限り早く我々のTSSがロードされて、CPUは有効な割り込みスタック・テーブル（IST)にアクセスできます。
   そして、我々のダブル・フォルトIDTエントリを修正することにより、CPUが我々の新しいダブル・フォルトスタックを使用するべきであることを、CPUに伝えることができます。

最初の2つの手順のために、我々の`gdt::init`関数の中で、`code_selector`と`tss_selector`変数にアクセスする必要があります。
新しい`Selector`構造体を通じて、それらを静的変数の一部にすることでこれを実施できます。

```rust
// in src/gdt.rs

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));

        (
            gdt,
            Selectors {
                code_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}
```

現時点で、`cs`レジスタと我々の`TSS`をロードするために、セレクタを使用できます。

```rust
// in src/gdt.rs

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        load_tss(GDT.1.tss_selector);
    }
}
```

[`set_cs`](https://docs.rs/x86_64/0.14.2/x86_64/instructions/segmentation/fn.set_cs.html)を使用してコード・セグメント・レジスタをリロードして、[`load_tss`](https://docs.rs/x86_64/0.14.2/x86_64/instructions/tables/fn.load_tss.html)を使用してTSSをロードします。
それらの関数は`unsafe`としてマークされているため、それらを呼び出すために`unsafe`ブロックが必要です。
その理由は、それが不正なセレクタをロードすることによって、メモリ安全性を破壊する可能性があるかもしれないからです。

現時点で、有効なTSSと割り込みスタック・テーブルをロードしたため、IDT内の我々のダブル・フォルトハンドラのためにスタックのインデックスを設定できます。

```rust
// in src/interrupts.rs

use crate::gdt;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
        }

        idt
    };
}
```

`set_stack_index`メソッドは、呼び出し側は使用されたインデックスが有効で、ほかの例外によって既に利用されていないことを保証しなければならないため、アンセーフです。

それでおしまいです！
現在、CPUはダブル・フォルトが発生したときはいつでもダブル・フォルト・スタックを切り替えます。
従って、カーネルのスタック・オーバーフローを含めて、*すべて*のダブル・フォルトを受け取れます。

![catching all double faults](https://os.phil-opp.com/double-fault-exceptions/qemu-double-fault-on-stack-overflow.png)

今後、再度トリプル・フォルトを見ることは決してありません！
上記を不注意で破壊しないことを保証するために、このテストを追加するべきです。

## スタック・オーバーフロー・テスト

我々の新しい`gdt`モジュールをテストして、スタック・オーバーフローでダブル・フォルトハンドラが正しく呼ばれることを保証するために、統合テストを追加できます。
そのアイデアは、テスト関数でダブル・フォルトを引き起こして、ダブル・フォルト・ハンドラが呼び出されたことを検証することです。

最小の骨組みから始めましょう。

```rust
// in tests/stack_overflow.rs

#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    blog_os::test_panic_handler(info)
}
```

我々の`panic_handler`テストのように、そのテストは[テスト・ハーネスなし](https://os.phil-opp.com/testing/#no-harness-tests)で実行します。

その理由は、ダブル・フォルトのあとで実行を継続できないため、1つ以上のテストは意味がありません。
そのテストのためのテスト・ハーネスを無効にするために、次を我々の`Cargo.toml`に追加します。

```toml
// in Cargo.toml

[[test]]
name = "stack_overflow"
harness = false
```

現時点で、`cargo test --test stack_overflow`はコンパイルに成功します。
もちろん、`unimplemented`マクロがパニックするため、テストは失敗します。

### `_start`の実装

`_start`関数の実装はこのように見えます。

```rust
// in tests/stack_overflow.rs

use blog_os::serial_print;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    blog_os::gdt::init();
    init_test_idt();

    // trigger a stack overflow
    stack_overflow();

    panic!("Execution continued after stack overflow");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow(); // for each recursion, the return address is pushed
    volatile::Volatile::new(0).read(); // prevent tail recursion optimaization
}
```

新しいGDTを初期化するために我々の`gdt::init`関数を呼び出しています。
我々の`interrupts::init_idt`関数を呼び出す代わりに、すぐに説明する`init_test_idt`関数を呼び出します。
その理由は、パニックする代わりに`exit_qemu(QemuExitCode::Success)`を実行する独自のダブル・フォルト・ハンドラを登録したいからです。

`stack_overflow`関数は、我々の`main.rs`内の関数とほとんど同じです。
唯一の違いは、関数の末尾で、[末尾呼び出し省略(tail call elimination)](https://en.wikipedia.org/wiki/Tail_call)と呼ばれるコンパイラの最適化を防ぐために、[`Volatile`](https://docs.rs/volatile/0.2.6/volatile/struct.Volatile.html)を使用して、追加の[volatile](https://en.wikipedia.org/wiki/Volatile_(computer_programming))読み込みを実行することです。
とりわけ、この最適化は、コンパイラが、最後の文が普通のループに入る再帰関数呼び出しである関数を変形することを許可します。
従って、関数呼び出して追加のスタック・フレームが作成されず、スタックの使用量は一定のままになります。

しかし、我々の場合、スタック・オーバーフローの発生を望むため、ダミーで揮発性読み込み文を関数の最後に追加して、コンパイラが削除することを許されないようにしました。
従って、その関数は*末尾再帰*ではなく、ループへの変換が防止されます。
関数が永遠に再帰することを報告するコンパイラの警告を抑制するために、`allow(unconditional_recursion)`属性も追加しています。

### IDTのテスト

上記で注意したように、テストは独自のダブル・フォルト・ハンドラを持つ、それ自身のIDTを必要とします。
その実装はこのように見えます。

```rust
// in tests/stack_overflow.rs

use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(blog_os::gdt::DOUBLE_FAULT_IST_INDEX);
        }

        idt
    };
}

pub fn init_test_idt() {
    TEST_IDT.load();
}
```

実装が`interrupts.rs`内の通常のIDTととても似ています。
通常のIDT内のように、分離したスタックに切り替えするために、ダブル・フォルト・ハンドラのために、IST内のスタック・インデックスを設定します。
`init_test_idt`カンスは`load`メソッドを通じてCPUのIDTをロードします。

### ダブル・フォルト・ハンドラ

唯一の足りない断片は我々のダブル・フォルト・ハンドラです。
それはこのように見えます。

```rust
#![feature(abi_x86_interrupt)]

use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use blog_os::{exit_qemu, serial_print, serial_println, QemuExitCode};

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);

    loop {}
}
```

ダブル・フォルト・ハンドラが呼ばれたとき、成功終了コードでQEMUを終了して、通過したテストとしてマークします。
統合テストは完全に分離された実行形式であるため、再度、我々のテストファイルの最も上に`#![feature(abi_x86_interrupt)]`属性を置く必要があります。

現時点で、`cargo test --test stack_overflow`（または、すべてのテストを実行する
`cargo test`）を通じて、我々のテストを実行できます。
予期したように、コンソール内に`stack_overflow... [ok]`を確認できます。
`set_stack_index`の行をコメントアウトしてみてください。それがテストに失敗する原因のはずです。

## まとめ

この投稿では、ダブル・フォルトが何であるか、またどの条件下でそれが発生するか学びました。
エラーメッセージを出力する基本的なダブル・フォルトハンドラを追加して、そのための統合テストを追加しました。

スタック・オーバーフローが発生したときも機能するように、ダブル・フォルトが発生したときにハードウェアがサポートするスタックを切り替えすることも可能です。
それを実装する間、タスク状態セグメント（TSS）、TSSに含まれた割り込みスタック・テーブル（IST)、そして古いアーキテクチャにおけるセグメンテーションで使用されたグローバル記述子テーブル（GDT）について学びました。

## 次は何でしょうか？

次の投稿は、タイマー、キーボードまたはネットワーク・コントローラーのような外部機器からの割り込みを処理する方法を説明します。
これらのハードウェア割り込みは、例えば、それらもIDTを通じて割り当てされるなど、例外ととても似ています。
しかし、例外と異なり、それらは直接CPUで発生しません。
代わりに、*割り込みコントローラー*がこれらの割り込みを集めて、それらをその優先度に依存してCPUに送信します。
次の投稿では、[Intel 8259]()（"PIC"）割り込みコントローラーを探求して、キーボードのサポートを実装する方法を学びます。
