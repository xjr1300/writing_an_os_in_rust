# ハードウェア割り込み

この投稿において、正確にハードウェア割り込みをCPUに送信する、プログラム可能な割り込みコントローラーを準備します。
これらの割り込みを処理するために、ちょうど我々の例外処理で実施したように、我々の割り込み記述子テーブルに新しいエントリを追加します。
定期的なタイマー割り込みを受け取るする方法と、キーボードからの入力を受け取る方法を学びます。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/hardware-interrupts/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-07`](https://github.com/phil-opp/blog_os/tree/post-07)ブランチで見つけることができます。

## 概要

割り込みは、接続されているハードウェア機器からCPUに通知する方法です。
カーネルに新しい文字がないかキーボードを定期的に確認させる代わりに（ポーリングと呼ばれる処理）、キーボードはそれぞれのキーの押下について、カーネルに通知できます。
カーネルは、何かが発生したとき行動する必要があるため、これはよりとても効率的です。
また、カーネルは次のポーリング時だけでなく、すぐに反応できるため、反応時間を短縮できます。

CPUにすべてのハードウェア機器を直接接続することは不可能です。
代わりに、分離した*割り込みコントローラー*は、すべての機器からの割り込みを集めて、CPUに通知します。

```
                                    ____________             _____
               Timer ------------> |            |           |     |
               Keyboard ---------> | Interrupt  |---------> | CPU |
               Other Hardware ---> | Controller |           |_____|
               Etc. -------------> |____________|
```

ほとんどの割り込みコントローラーはプログラム可能で、それは割り込みおためにさまざまな優先レベルをサポートすることを意味します。
例えば、これは、正確に時間を守ることを保証するために、タイマー割り込みをキーボード割り込みよりも高い優先度を与えることができます。

例外と異なり、ハードウェア割り込みは*非同期*で発生します。
これは、それらが実行しているコードから完全に独立して、いつでも発生できることを意味します。
従って、潜在的な並行処理関連のすべてのバグを含む、カーネル内の並行処理が突然発生します。
Rustの所有権モデルの制約は、可変なグローバル状態を禁じるため、ここで役に立ちます。
しかし、この投稿の後半で確認するように、依然としてデッドロックする可能性があります。

## 8259 PIC

[Intel 8259](https://en.wikipedia.org/wiki/Intel_8259)は、1976年に導入されたプログラム可能な割り込みコントローラ（PIC）です。
それは、より新しい[APIC](https://en.wikipedia.org/wiki/Intel_APIC_Architecture)に長い期間をかけて置き換えられましたが、そのインターフェースは後方互換性を理由に、依然として現在のシステムでサポートされています。
8259 PICはAPICよりも準備することがはるかに簡単なため、後の投稿でAPICに切り変えるするまで、それを使用して割り込みについて説明します。

8259は8本の割り込み線とCPUと会話するためのいくつかの線を持っています。
当時の典型的なシステムには、8259PICの2つのインスタンス、1つのプライマリと1つのセカンダリPIC、が装備されており、プライマリの1つの割り込み線に接続されていました。

```
                     ____________                          ____________
Real Time Clock --> |            |   Timer -------------> |            |
ACPI -------------> |            |   Keyboard-----------> |            |      _____
Available --------> | Secondary  |----------------------> | Primary    |     |     |
Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
Primary ATA ------> |            |   Floppy disk -------> |            |
Secondary ATA ----> |____________|   Parallel Port 1----> |____________|
```

このグラフは典型的な割り込み線の割り当てを示しています。
例えばセカンダリPICのライン4はマウスに割り当てられるなど、15本の先の殆どが固定されたマッピングを持っていることを確認できます。

それぞれのコントローラーは2つの[I/Oポート](https://os.phil-opp.com/testing/#i-o-ports)を通じて構成されており、それは1つの"コマンド"ポートと1つの"データ"ポートです。
プライマリ・コントローラーのために、これらのポートは`0x20`（コマンド）と`0x21`（データ）です。
セカンダリ・コントローラについて、それらは`0xa0` (コマンド）と`0xa1`（データ）です。
どのようにPICが構成されているかは、[osdev.orgにある記事](https://wiki.osdev.org/8259_PIC)を参照してください。

### 実装

PICのデフォルトの設定は、0から15の範囲の割り込みベクトル番号をCPUに送信するため、使用できません。
これらの番号は、CPU例外によって既に占有されています。
例えば、8番はダブル・フォルトに対応しています。
この重複の問題を修正するために、異なる番号にPICの割り込みを再割り当てする必要があります。
実際の範囲は例外と重複しない限り関係ありませんが、32の例外スロットの後の最初の空き番号であるため、典型的に32から47までの範囲が選択されます。

設定は、PICのコマンド及びデータ・ポートに特別な値を書き込むことによってできます。
幸運にも、既に[`pic8259`](https://docs.rs/pic8259/latest/pic8259/)と呼ばれるクレートがあるため、我々自身で初期化シーケンスを書き込む必要はありません。
しかしながら、それがどのように機能するか興味がある場合、[そのソース・コード](https://docs.rs/crate/pic8259/0.10.1/source/src/lib.rs)を参照してください。
それは小さく、そして十分にドキュメント化されています。

依存関係にそのクレートを追加するために、次を我々のプロジェクトに追加します。

```toml
# in Cargo.toml

[dependencies]
pic8259 = "0.10.1"
```

そのクレートによって提供される主要な抽象化は、上で見たプライマリ／セカンダリPICレイアウトを表現する[`ChainedPics`](https://docs.rs/pic8259/0.10.1/pic8259/struct.ChainedPics.html)構造体です。
それは、次の方法で仕様されるために設計されています。

```rust
// in src/interrupts.rs

use spin;

use pic8259::ChainedPics;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
```

上記の通り、PICのオフセットを32から47の範囲に設定しています。
`Mutex`内に`ChainedPics`構造体を覆うことにより、（[`lockメソッド`](https://docs.rs/spin/0.5.2/spin/struct.Mutex.html#method.lock)を通じて）次の手順で必要になる、安全な可変アクセスができます。
`ChainedPics::new`関数は、不正なオフセットが未定義の振る舞いを起こす可能性があるため、アンセーフです。

現段階で、我々の`init`関数内で、8259PICを初期化できます。

```rust
// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };    // new
}
```

我々はPICの初期化を実行するために、[`initialize`](https://docs.rs/pic8259/0.10.1/pic8259/struct.ChainedPics.html#method.initialize)関数を使用しました。
`ChainedPics::new`関数と同様に、この関数も、PICが設定ミスされていた場合、未定義な振る舞いを起こす可能性があるためアンセーフです。

すべてがうまくいけば、`cargo run`を実行したとき、"It did not crash"メッセージを確認できるはずです。

## 割り込みの有効化

現在まで、割り込みがCPUの設定で未だ無効になっているため、何も起こりませんでした。
これは、CPUが全く割り込みコントローラーを確認していないことを意味しているため、割り込みはCPUに届きません。
それを変更しましょう。

```rust
// in src/lib.rs

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();     // new
}
```

`x86_64`くれとの`interrupts::enable`関数は、外部割り込みを有効にするために、（"割り込みを設定する"）特別な`sti`命令を実行します。
現時点で、`cargo run`を試行したとき、ダブル・フォルトが発生することを確認できます。

![double fault occured](https://os.phil-opp.com/hardware-interrupts/qemu-hardware-timer-double-fault.png)

ダブル・フォルトが発生した理由は、ハードウェア・タイマー（正確には[Intel 8253](https://en.wikipedia.org/wiki/Intel_8253)）がデフォルトで有効になっているため、割り込みを有効にしてからすぐに、タイマー割り込みの受け取りを開始したからです。

タイマー割り込みのハンドラ関数を定義していないため、ダブル・フォルト・ハンドラが呼び出されました。

## タイマー割り込みの処理

[上記](https://os.phil-opp.com/hardware-interrupts/#the-8259-pic)のグラッフィックの通り、タイマーはプライマリPICのライン0を使用しています。
これは、タイマー割り込みが割り込み32（0 + 32オフセット）としてCPUに届いたことを意味しています。
インデックス32をハードコーディングすることに代わり、それを`InterruptIndex`列挙型に蓄積します。

```rust
// in src/interrupts.rs

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        self as usize
    }
}
```

その列挙型は[Cのような列挙型](https://doc.rust-lang.org/reference/items/enumerations.html#custom-discriminant-values-for-fieldless-enumerations)であるため、それぞれのバリアントのインデックスを直接指定することができます。
`repr(u8)`属性は、それぞれのバリアントが`u8`として表現されることを指定しています。
そのうちに、他の割り込みのためにより多くのバリアントを追加する予定です。

現段階で、我々はタイマー割り込みのハンドラ関数を追加できます。

```rust

use crate::print;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX); // new
        }
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);      // new

        idt
    };
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");
}
```

`timer_interrupt_handler`は、CPUは例外と外部割り込みに同じように反応するため（唯一の違いは、いつくかの例外がエラー・コードを追加することです）、我々の例外と同じシグネチャーを持ちます。
[`InterruptDescriptroTable`](https://docs.rs/x86_64/0.14.2/x86_64/structures/idt/struct.InterruptDescriptorTable.html)構造体は[`IndexMut`](https://doc.rust-lang.org/core/ops/trait.IndexMut.html)トレイトを実装するため、配列インデックス構文を通じて、高尾のエントリにアクセスできます。

我々のタイマー割り込みハンドラーにおいて、スクリーンにドットを出力します。
タイマー割り込みが定期的に発生するため、それぞれのタイマの刻みでドットの出現を確認することを期待します。
しかしながら、それを実行したとき、たった1つのドットのみ出力されます。

![only one dot](https://os.phil-opp.com/hardware-interrupts/qemu-single-dot-printed.png)

### 割り込みの終了

その理由は、我々の割り込みハンドラからのPICは明示的な"割り込み終了（EOI）信号を予期しているからです。
この信号は、割り込みが処理されシステムが次の割り込みを受け取る準備ができていることを、コントローラーに伝えます。
よって、PICは最初のタイマー割り込みの処理に忙しいと考え、次の割恋を送る前に、EOI信号を根気強く待つからです。

EOIを送信するために、再度我々の静的な`PICS`構造体を使用します。

```rust
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
```

`notify_end_of_interrupt`は、プライマリまたはセカンダリPICのうち、どちらが割り込みを送信したかどうかを判断して、それぞれのコントローラにEOI信号を送信するために、`コマンド`と`データ`ポートを使用します。
もしセカンダリPICが割り込みを送信した場合、セカンダリPICはプライマリPICの入力線に接続されているため、両方のPICは通知される必要があります。

我々は正しい割り込みベクタ番号を使用するように注意する必要があり、そうしなければ、重要な未送信の割り込みを不注意で削除するか、我々のシステムの停止を引き起こす可能性があります。
これが、その関数がアンセーフである理由です。

現時点で`cargo run`を実行したとき、スクリーン上に定期的に現れるドットを確認できます。

![dots appear periodically on the screen](https://os.phil-opp.com/hardware-interrupts/qemu-hardware-timer-dots.gif)

### タイマーの設定

我々が使用するハードウェア・タイマーは*プログラム可能なインターバル・タイマー*、または省略してPITと呼ばれています。
名前の通り、それは2つの割り込みの間隔を設定することができます。
すぐに[APICタイマー](https://wiki.osdev.org/APIC_timer)に切り替える予定があるため、ここで詳細を示しませんが、OSDevウィキは[PITの設定](https://wiki.osdev.org/Programmable_Interval_Timer)について広範囲な投稿があります。

## デッドロック

現在、我々のカーネル内に同時実行の形式ができました。
タイマー割り込みは非同期で発生するため、それらはいつでも我々の`_start`関数に割り込みできます。
幸運にも、Rustの所有権システムはコンパイル時に多くの種類の同時実行に関連したバグを防止します。
注目に値する例外の1つはデッドロックです。
スレッドが決して開放されないロックの要求を試みた場合にデッドロックが発生します。
従って、スレッドは無期限に停止します。

我々は既に我々のカーネル内でデッドロックを引き起こすことができます。
`println`マクロが、スピンロックを使用して[グローバルな`WRITER`をロック](https://os.phil-opp.com/vga-text-mode/#spinlocks)する`vga_buffer::print`関数を呼びだすことを思い出してください。

```rust
// in src/vga_buffer.rs

[...]

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

それは、`write_fmt`を呼び出して`WRITER`をロックして、関数の終了位置でそれを暗黙的に開放します。
`WRITER`がロックされている間に割り込みが発生して、割り込みハンドラも何かを出力することを試みることを想像してください。

| ステップ | _start関数                | 割り込みハンドラ                                        |
| -------- | ------------------------- | ------------------------------------------------------- |
| 0        | `println!`呼び出し        | ---                                                     |
| 1        | `print`が`WRITER`をロック | ---                                                     |
| 2        | ---                       | **割り込みが発生**して、ハンドラが実行を開始            |
| 3        | ---                       | `println!`呼び出し                                      |
| 4        | ---                       | `print`が（既にロックされている）`WRITER`のロックを試行 |
| 5        | ---                       | `print`が（既にロックされている）`WRITER`のロックを試行 |
| ...      | ---                       | ...                                                     |
| 発散     | `WRITER`が開放されない    | ...                                                     |

`WRITER`がロックされているため、割り込みハンドラはそれが開放されるまで待ちます。
しかし、割り込みハンドラが戻った後、`_start`関数のみ実行を継続出来るため、ロックの解放は決して起こりません。
従って、システム全体が停止します。

### デッドロックを引き起こす

`_start`関数の終わりにあるルーっ府内で、何かを出力することによって、我々のカーネル内にそのようなデッドロックを簡単に引き起こすことができます。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    [...]

    loop {
        use blog_os::print; // new
        print!("-"); // new
    }
}
```

![deadlock](https://os.phil-opp.com/hardware-interrupts/qemu-deadlock.png)

最初のタイマー割り込みが発生するまで、限定された数だけのハイフンが出力されることを確認できます。
次に、タイマー割り込みハンドラがドットを出力することを試行したとき、タイマー割り込みハンドラがデッドロックするため、システムが停止します。
これが、上記の出力にドットがない理由です。

### デッドロックの修正

デッドロックを避けるために、`Mutex`がロックされている限り、割り込みを無効にできます。

```rust
/// Prints the given formatted string to the VGA text buffer
/// through the global `WRITER` instance.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts; // new

    interrupts::without_interrupts(|| { // new
        WRITER.lock().write_fmt(args).unwrap();
    });
}
```

[`without_interrupt`](https://docs.rs/x86_64/0.14.2/x86_64/instructions/interrupts/fn.without_interrupts.html)関数は、[クロージャー](https://doc.rust-lang.org/book/ch13-01-closures.htmlを受け取り、割り込みのない環境でクロージャーを実行します。
`Mutex`がロックされている限り割り込みが発生できないことを保証するためにそれ（`without_interrupt`関数）を使用します。
現段階で我々のカーネルを起動したとき、それ（カーネル）が停止しないで実行し続けることを確認できます。
（未だにドットを見ることはありませんが、これはスクロールが早すぎるからです。例えば、ループ内に`for _ in 0..10000 {}`を置くことで、出力を遅くしてみましょう。）

デッドロックが発生しないことを保証するために、シリアル出力関数にも同じ変更を適用できます。

```rust
// in src/serial.rs

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts; // new

    interrupts::without_interrupts(|| { // new
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    });
}
```

割り込みを無効にすることは、一般的な解決であるべきではないことに注意してください。
問題は、例えば、システムが割り込みに対して反応するまでの時間など、最悪の場合の割り込み遅延が増加することです。
よって、割り込みは、非常に短い時間の間だけ無効にするべきです。

## 競合状態の修正

`cargo run`を実行した場合、`test_println_output`テストが失敗することを確認するかもしれません。

```bash
> cargo test --lib
[…]
Running 4 tests
test_breakpoint_exception...[ok]
test_println... [ok]
test_println_many... [ok]
test_println_output... [failed]

Error: panicked at 'assertion failed: `(left == right)`
  left: `'.'`,
 right: `'S'`', src/vga_buffer.rs:205:9
 ```

その理由は、テストと我々のタイマーハンドラ間の競合状態です。
テストが次のようであったことを思い出してください。

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    println!("{}", s);
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer
            .chars[BUFFER_HEIGHT - 2][i].read();
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
```

テストは、VGAバッファに文字列を出力して、次に`buffer_chars`配列を手動で順に走査することで出力を確認します。
タイマー割り込みハンドラは`println`とスクリーンの文字の読み込みの間に実行うされるかもしれないため、競合状態が発生します。
これはRustがコンパイル時に完全に防ぐ危険な*データ競合*でないことに注意してください。
詳細は、[*Rustonomicon*](https://doc.rust-lang.org/nomicon/races.html)を参照してください。

これを修正するために、タイマー・ハンドラがスクリーンに`.`を出力できないようにするために、すべてのテストの間、`WRITER`ロックを維持する必要があります。
修正されたテストは次のようになります。

```rust
// in src/vga_buffer.rs

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "Some test string that fits on a single line";
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
```

次を変更しました。

- 明示的に`lock()`メソッドを使用することで、すべてのテストの間、ライター・ロックを維持します。
  `println`の代わりに、既にロックされたライターへの書き込みを許可する[`writeln`](https://doc.rust-lang.org/core/macro.writeln.html)マクロを使用します。
- 他のデッドロックを防止するために、テストの間、割り込みを無効にしました。
  そうでない場合、テストはライターがロックされている間に、割り込みを得るかもしれません。
- タイマー割り込みハンドラはテストの前に実行される可能性があるため、文字列`s`を出力する前に追加の改行`\n`を出力します。
  この方法で、既にタイマーハンドラが現在行にいくつかの`.`文字を出力したときに、テストに失敗しないようにします。

上記の変更で、現在、`cargo test`は再び決定的に（テストに）成功します。

これは、テストの失敗を引き起こすのみの、非常に無害な競合状態です。
想像したように、他の競合状態はそれらの非決定的な性質により、デバッグがとても困難になる可能性があります。
幸運にも、Rustは、システム破壊や静かなメモリ破壊を含む、すべての種類の未定義な動作を引き起こす最も深刻な場合の競合状態であるデータ競合から我々を避けてくれます。

## `hlt`命令

現在まで、単純なからのループ分を我々の`_start`や`panic`関数の最後に使用しました。
これは、終わりのないスピンをCPUに引き起こし、それにより予期したとおり機能します。
しかし、それは、なにもすることがない間もフル・スピードで動作し続けるため、とても非効率です。
カーネルを実行したとき、タスク・マネージャーでこの問題を確認できます。
QEMUプロセスは、常に100%近くのCPUを必要とします。

本当にやりたいことは、次の割り込みが到着するまで、CPUを停止することです。
これにより、CPUはスリープ状態に入り、エネルギー消費を大幅に減らします。
[`hlt`命令](https://en.wikipedia.org/wiki/HLT_(x86_instruction))は、それを正確に実行します。
この命令をエネルギー効率の良い無限ループを作成するために使いましょう。

```rust
// in src/lib.rs

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
```

`instructions::hlt`関数は、アセンブリ命令の周りを[薄く覆った](https://github.com/rust-osdev/x86_64/blob/5e8e218381c5205f5777cb50da3ecac5d7e3b1ab/src/instructions/mod.rs#L16-L22)だけです。
それは、メモリの安全性を損なう可能性がないため安全です。

現時点で、我々の`_start`と`panic`関数内の無限ループの代わりに、この`hlt_loop`を使用できます。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    […]

    println!("It did not crash!");
    blog_os::hlt_loop();            // new
}


#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    blog_os::hlt_loop();            // new
}
```

同様に我々の`lib.rs`を更新しましょう。

```rust
// in src/lib.rs

/// Entry point for `cargo test`
#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();
    hlt_loop();         // new
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();         // new
}
```

QEMEに我々のカーネルを起動したとき、とても少ないCPU使用率を確認できます。

## キーボード入力

現時点で、外部機器からの割り込みを処理することができるため、最後にキーボード入力のサポートを追加します。
これは、初めて我々のカーネルと相互作用できるようになります。

> ここでは、*USB*キーボードではなく[PS/2](https://en.wikipedia.org/wiki/PS/2_port)キーボードを処理する方法のみを説明することに注意してください。
> しかし、メインボードは、古いソフトウェアをサポートするために、USBキーボードをPS/2機器として模倣するため、我々のカーネルがUSBをサポートするまで安全にUSBキーボードをキーボードを無視できます。

ハードウェア・タイマーのように、キーボード・コントローラーは、既にデフォルトで有効になっています。
よって、キーを押したとき、キーボード・コントローラーは、PICに割り込みを送信して、PICはそれをCPUに転送します。
CPUはIDT内のハンドラ関数を探しますが、対応するエントリは空です。
よって、ダブル・フォルトが発生します。

よって、キーボード割り込み用のハンドラ関数を追加しましょう。
それは、タイマー割り込み用のハンドラを定義した方法と非常に似ています。
それは、ただ異なる割り込み番号を使用します。

```rust
// in src/interrupts.rs

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard, // new
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        […]
        // new
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!("k");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

[上記](https://os.phil-opp.com/hardware-interrupts/#the-8259-pic)グラフィックで確認出来るように、キーボードはプライマリPICのライン1を使用します。
これは、割り込み33（1 + 32オフセット）としてCPUに届きます。
`InterruptIndex`列挙型に`Keyboard`バリアントを追加することで、このインデックスを追加します。
列挙型は前の値に1を加えた値がデフォルトであるため、明示的に値を指定する必要はありません。
割り込みハンドラ内で、`k`を出力して、割り込みコントローラーに命令終了信号を送信します。

現時点で、キーボードを押したときスクリーンに`k`が表示されることを確認できます。
キーを押し続けた場合でも、これ以上スクリーンに`k`が表示されません。
これは、キーボード・コントローラーが、押したキーのいわゆる*スキャンコード*を読み込むまで、他の割り込みを送信しないためです。

### スキャンコードの読み込み

*どの*キーが押されたか見つけるために、キーボードコントローラに問い合わせする必要があります。
我々は、番号`0x60`の[I/Oポート](https://os.phil-opp.com/testing/#i-o-ports)である、PS/2コントローラーのデータポートから読み込みすることでこれを実施します。

```rust
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    print!("{}", scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

我々はキーボードのデータ・ポートからバイトを読むために`x86_64`クレートの[`Port`](https://docs.rs/x86_64/0.14.2/x86_64/instructions/port/struct.Port.html)型を使用します。
このバイトは[*スキャンコード*](https://en.wikipedia.org/wiki/Scancode)と呼ばれ、それは押した／離したキーを表現します。
我々は、未だにスクリーンにスキャンコードを出力する以外、何もしていません。

![print a scancode](https://os.phil-opp.com/hardware-interrupts/qemu-printing-scancodes.gif)

上記の画像は"123"とゆっくりタイプしたことを示しています。
隣接したキーは隣接したスキャンコードを持ち、キーを押すと離すのとは異なるスキャンコードが発生することがわかります。
しかし、どのように、スキャンコードを実際のキーとアクションを正確に翻訳するのでしょうか？

### スキャンコードの解釈

スキャンコードとキーのマッピングには3つの異なる標準があり、*スキャンコード・セット*と呼ばれています。
3つすべては初期のIBMコンピューターのキーボードまでさかのぼり、それらは[IBM XT](https://en.wikipedia.org/wiki/IBM_Personal_Computer_XT)、[IBM 3270 PC](https://en.wikipedia.org/wiki/IBM_3270_PC)及び[IMB AT](https://en.wikipedia.org/wiki/IBM_Personal_Computer/AT)です。
幸運にも、後期のコンピューターは、新しいスキャンコード・セットの定義の取り扱いを継続しませんでしたが、既存のセットを模倣して拡張しました。
現在、ほとんどのキーボードは3つのセットのいずれかを模倣するように設定できます。

デフォルトでは、PS/2キーボードはスキャンコード・セット1（"XT"）を模倣しています。
このセットにおいて、スキャンコード・バイトの下位7ビットはキーを定義して、最大ビットは押した（"0"）か、離した（"1"）かどちらかを定義します。
キーパッド上のエンター・キーのように、オリジナルな[IBM XT](https://en.wikipedia.org/wiki/IBM_Personal_Computer_XT)キーボードに存在しないキーは、2つのキーコードを連続して生成します。
`0xe0`エスケープ・バイトと次のキーを表現するバイトです。
セット1のスキャンコードのすべてのリストとそれらに対応するキーは、[OSDev Wiki](https://wiki.osdev.org/Keyboard#Scan_Code_Set_1)を参照してください。


スキャンコードをキーに翻訳するために、`match`文を使用できます。

```rust
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    let key = match scancode {
        0x02 => Some('1'),
        0x03 => Some('2'),
        0x04 => Some('3'),
        0x05 => Some('4'),
        0x06 => Some('5'),
        0x07 => Some('6'),
        0x08 => Some('7'),
        0x09 => Some('8'),
        0x0a => Some('9'),
        0x0b => Some('0'),
        _ => None,
    };
    if let Some(key) = key {
        print!("{}", key);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

上記のコードは、数字キー'0'から'9'のキーの押下を翻訳して、すべての他のキーを無視します。
それ（上記のコード）は、それぞれのスキャンコードに対する文字または`None`を割り当てます。
次に、それ（上記のコード）は、オプショナルな`key`が内包する値を取り出すために[`if let`](https://doc.rust-lang.org/book/ch18-01-all-the-places-for-patterns.html#conditional-if-let-expressions)を使用します。
パターン内で同じ変数名`key`を使用することで、前の宣言を[隠し](https://doc.rust-lang.org/book/ch03-01-variables-and-mutability.html#shadowing)ます。これはRustにおいて`Option`型から内包する値を取り出すための、一般的なパターンです。

現時点で、数字を書き込めます。

![writing a number](https://os.phil-opp.com/hardware-interrupts/qemu-printing-numbers.gif)

同じ方法で他のキーを翻訳できます。
幸運にも、スキャンコード・セット1と2のスキャンコードを翻訳する`pc-keyboard`と呼ばれるクレートがあるため、これを我々自身で実装する必要はありません。
そのクレートを使用するために、それを`Cargo.toml`に追加して、我々の`lib.rs`にインポートします。

```rust
// in src/interruput.rs

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Jis109Key, ScancodeSet1>> = Mutex::new(
            Keyboard::new(layouts::Jis109Key, ScancodeSet1, HandleControl::Ignore)
        );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}
```

Mutexに保護された静的な[`Keyboard`](https://docs.rs/pc-keyboard/0.5.0/pc_keyboard/struct.Keyboard.html)オブジェクトを作成するために`lazy_static`マクロを使用します。
`Keyboard`をUSキーボード・レイアウト（本ドキュメントにおいてはJIS109キーボード・レイアウト）とスキャンコード・セット1で初期化します。
[`HandleControl`](https://docs.rs/pc-keyboard/0.5.0/pc_keyboard/enum.HandleControl.html)パラメーターは、`U_001A`を通じてユニコード文字`U+0001`に`Ctrl+[a-z]`をマップさせます。
そのようにしたくない場合は、`ctrl`を通常のキーのように扱う`Ignore`オプションを使用します。

それぞれの割り込みにおいて、Mutexをロックして、キーボード・コントローラーからスキャンコードを読み取り、そしてそれを（スキャンコード）を、スキャンコードを`Option<KeyEvent>`に翻訳する[`add_byte`](https://docs.rs/pc-keyboard/0.5.0/pc_keyboard/struct.Keyboard.html#method.add_byte)メソッドに渡します。
[`KeyEvent`](https://docs.rs/pc-keyboard/0.5.0/pc_keyboard/struct.KeyEvent.html)は、イベントを起こしたキーと、それが押されたかまたは離されたかどちらかのイベントを含みます。

このキー・イベントを解釈するために、それを可能であればキー・イベントを文字に翻訳する[`process_keyevent`](https://docs.rs/pc-keyboard/0.5.0/pc_keyboard/struct.Keyboard.html#method.process_keyevent)メソッドに渡します。
例えば、それは`A`キーが押されたイベントを、シフト・キーが押されているかどちらかに依存して、小文字の`a`文字または大文字の`A`文字のどちらかに翻訳します。

この修正した割り込みハンドラで、現在、テキストを書き込めます。

![writing a text](https://os.phil-opp.com/hardware-interrupts/qemu-typing.gif)

### キーボードの設定

例えば、キーボードがどのスキャンコード・セットを使用するべきかなど、PS/2キーボードのいくつかの側面を設定できます。
この投稿は十分に長いため、ここでそれを説明しませんが、OSDev Wikiは利用できる[設定コマンド](https://wiki.osdev.org/PS/2_Keyboard#Commands)の概要があります。

## まとめ

この投稿でどのように外部割り込みを有効にして処理するかを説明しました。
我々は8259PICとそのプライマリ／セカンダリ・レイアウト、割り込み番号の再マッピング、そして"割り込み終了"信号を学びました。
我々は、ハードウェア・タイマーとキーボードの処理を実装して、次の割り込みまでCPUを停止する`hlt`命令について学びました。

現在、我々は我々のカーネルと相互作用できるようになり、小さなシェルや単純なゲームを作成するための、基本的な構成要素（building blocks）があります。

## 次は何でしょうか？

タイマー割り込みは実行しているプロセスに定期的に割り込み、カーネルが制御を取り戻す方法を提供するため、オペレーティング・システムにとって不可欠です。
そして、カーネルは異なるプロセスに切り替えて、並行で複数のプロセスを実行する幻想を作り上げます。

しかし、プロセスやスレッドを作成する前に、それらのためにメモリを確保する方法が必要です。
次の投稿は、この基本的な構成要素が提供するメモリ管理について探求する予定です。
