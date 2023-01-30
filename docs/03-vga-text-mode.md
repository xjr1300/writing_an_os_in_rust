# VGAテキスト・モード

[VGAテキスト・モード](https://en.wikipedia.org/wiki/VGA-compatible_text_mode)はスクリーンにテキストを表示する簡単な方法です。
この投稿において、分離したモジュール内にすべての危険性を閉じ込めることにより、安全で簡単に利用できるインターフェースを作成します。
また、Rustの[書式マクロ]()をサポートを実装します。

この投稿は[GitHub](https://github.com/phil-opp/blog_os)で公開して開発されています。
もし、何らかの問題や質問があれば、そこに問題（issue）を発行してください。
また、[下に](https://os.phil-opp.com/vga-text-mode/#comments)コメントを残すこともできます。
この投稿の完全なソースコードは、[`post-03`](https://github.com/phil-opp/blog_os/tree/post-03)ブランチで見つけることができます。

## VGAテキスト・バッファ

VGAテキスト・モードでスクリーンに文字を表示するために、誰かがVGAハードウェアのテキストバッファにそれを書き込む必要があります。
VGAテキスト・バッファは、典型的に25行80列を持つ二次元配列で、それはスクリーンに直接描画されます。
それぞれの配列エントリは次の書式を通じて1つのスクリーンの文字を示します。

| ビット | 値                         |
| ------ | -------------------------- |
| 0-7    | アスキー・コード・ポイント |
| 8-11   | 前景色                     |
| 12-14  | 背景色                     |
| 15     | 点滅                       |

最初のバイトは[ASCIIエンコーディング](https://en.wikipedia.org/wiki/ASCII)で表示されるべき文字を表現します。
具体的には、それは正確にASCIIではありませんが、いくつか追加的な文字とわずかな変更を持つ、[コード・ページ437]()と名前が付けられたキャラクタ・セットです。
単純化するために、この投稿において、それをASCIIキャラクタと呼び続ける予定です。

２番目のバイトはどのように文字列を表示されるかを定義しています。
最初の４ビットは前景色、次の３ビットは背景色、そして最後のビットは文字を点滅するかを定義しています。
次の色が利用できます。

| 番号 | 色       | 番号 + 明るいビット | 明るい色   |
| ---- | -------- | ------------------- | ---------- |
| 0x0  | 黒       | 0x8                 | 暗灰       |
| 0x1  | 青       | 0x9                 | 水色       |
| 0x2  | 緑       | 0xa                 | 薄緑       |
| 0x3  | シアン   | 0xb                 | 薄いシアン |
| 0x4  | 赤       | 0xc                 | 薄赤       |
| 0x5  | マゼンダ | 0xd                 | ピンク     |
| 0x6  | 茶       | 0xe                 | 黄         |
| 0x7  | 薄灰     | 0xf                 | 白         |

ビット4は*明るいビット*で、例えば、それは青を水色に変換する。

```
0x0 (Black)       = 0x0000
0x8 (Dark Gray)   = 0x1000
0x1 (Blue)        = 0x0001
0x9 (Light Blue)  = 0x1001
0x2 (Green)       = 0x0010
0xa (Light Green) = 0x1010
0x3 (Cyan)        = 0x0011
0xb (Light Cyan)  = 0x1011
0x4 (Red)         = 0x0100
0xc (Light Red)   = 0x1100
                      ^
                      bit4
```

背景色のために、このビットは点滅ビットとして転用されています。

VGAテキスト・バッファは[メモリ・マップドI/O](https://en.wikipedia.org/wiki/Memory-mapped_I/O)を通じて、アドレス`0xb8000`にアクセス可能です。
これは、そのアドレスへの読み込みと書き込みはRAMにアクセスせずに、VGAハードウェアのテキストバッファに直接アクセスすることを意味しています。
これは、そのアドレスへの通常のメモリ操作を通じて、それを読み書き出来ることを意味しています。

メモリ・マップドハードウェアはすべての通常のRAM操作をサポートしていないことに注意してください。
例えば、デバイスがバイト単位の読み込みのみをサポートしており、`u64`が読み込まれたとき、壊れた値が返却されます。
幸運にも、テキストバッファは[通樹王の読み書きをサポート](https://web.stanford.edu/class/cs140/projects/pintos/specs/freevga/vga/vgamem.htm#manip)しているので、特別な方法でそれを取り扱う必要はありません。

## Rustモジュール

現在、どのようにVGAバッファを操作するかを理解したので、表示を制御するRustモジュールを作成することができます。

```rust
// in src/main.rs
mod vga_buffer;
```

このモジュールのコンテンツのために、新規に`src/vba_buffers.rs`ファイルを作成します。
以下のすべてのコードは、新しいモジュールに入ります（特に指定されていない限り）。

### 色

最初に、enumを使用して様々な色を表現します。

```rust
// in src/vga_buffer.rs
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}
```

それぞれの色の番号を明示的に記述するために、ここで[Cのような列挙型](https://doc.rust-lang.org/rust-by-example/custom_types/enum/c_like.html)を使用します。
`repr(u8)`属性の理由は、それぞれの列挙型のバリアン語が`u8`で保存されるようにするためです。
実際には4ビットで十分ですが、Rustには`u4`型がありません。

通常、コンパイラはそれぞれ使用されていないバリアントに警告を発します。
`#[allow(dead_code)]`属性を使用することで、`Color`列挙型に対するこれらの警告を無効にします。

[`Copy`](https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html)、[`Clone`](https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html)、[`Debug`](https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html)、[`PartialEq`](https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html)そして[`Eq`](https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html)トレイトを[派生](https://doc.rust-lang.org/rust-by-example/trait/derive.html)することで、その型の[意味的コピー](https://doc.rust-lang.org/1.30.0/book/first-edition/ownership.html#copy-types)を有効にして、その型を表示と比較できるようにします。

前景色と背景色を指定するすべての色のコードを表現するために、`u8`に[`newtype`](https://doc.rust-lang.org/rust-by-example/generics/new_types.html)を作成します。

> `newtype`イデオムは正しい型の値がプログラムに提供されることを、コンパイル時に保証します。
> 例えば、年齢をチェックする年齢検証関数には、`Years`型の値を指定する必要があります。

```rust
// newtype example
struct years(i64);

struct Days(i64);

impl Years {
    pub fn to_days(&self) -> Days {
        Days(self.0 * 365)
    }
}

impl Days {
    pub fn to_years(&self) -> Years {
        Years(self.0 / 365)
    }
}

fn old_enough(age: &Years) -> bool {
    age.0 >= 18
}

fn main() {
    let age = Years(5);
    let age_days = age.to_days();
    println!("Old enough {}", old_enough(&age));
    println!("Old enough {}", old_enough(&age_days.to_years()));
    // println!("Old enough {}", old_enough(&age_days));
}
```

```rust
// in src/vga_buffer.rs

/// u8の上位4ビットで背景色を、下位4ビットで前景色を管理
#[derive(Debug, Clone, Copy, PatialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foregraound: Color, background: Color) -> Self {
        Self ((background as u8) << 4 | (foreground as u8))
    }
}
```

`ColorCode`構造体はすべての色のバイトを含み、それは前景色と背景色を含んでいます。
前のように、それのために`Copy`と`Debug`トレイトを派生します。
`ColorCode`が`u8`と正確に同じデータ・レイアウトを持つことを保証するために、[`repr(transparent)`](https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent)属性を使用しています。

### テキスト・バッファ

現在、スクリーン文字とテキスト・バッファを表現する構造体を追加することができます。

```rust
// in src/vga_buffer.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```

Rustにおいて、デフォルトの構造体内のフィールドの順番は未定義であるため、[`repr(C)`](https://doc.rust-lang.org/nightly/nomicon/other-reprs.html#reprc)属性が必要です。
それは、構造体のフィールドがCの構造体と正確に同じように配置されることが保証されるため、正しいフィールドの順番が保証されます。
`Buffer`構造体のために、その1つのフィールドと同じメモリ配置を持つことを保証するために、再度[`repr(transparent)`](https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent)属性を使用します。

実際にスクリーンに書き出すために、ライター型を作成します。

```rust
// in src/vga_buffer.rs

pub struct Writer {
    column_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
}
```

ライターは常に最後の行を書き込み、行がいっぱいになったら（または`\n`で）行を上にシフトします。
`column_position`フィールドは、最終行の現在の位置を追跡し続けます。
現在の前景色と背景色は`color_code`で指定されており、VGAバッファへの参照は`buffer`内に保存されます。
[`'static`](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html#the-static-lifetime)ライフタイムは、プログラムが実行する全体の時間で参照が有効であることを示しています（VGAテキスト・バッファにおいては真実です）。

### 出力

現在、バッファの文字を変更するために`Writer`を使用できます。
まず、1つのASCIIバイトを書き込むメソッドを作成します。

```rust
// in src/vga_buffer.rs

mpl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = BUFFER_HEIGHT - 1;
                let col = self.column_position;
                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
            }
        }
    }

    pub fn new_line(&mut self) {
        unimplemented!();
    }
}
```

もし`byte`が[改行](https://en.wikipedia.org/wiki/Newline)バイト`\n`であった場合、ライターは何も出力しません。
代わりに、後で実装する予定の`new_line`メソッドを呼び出します。
他のバイトは2番目の`match`ケース内で、スクリーンに出力されます。

バイトを出力しているとき、ライターは現在行がいっぱいか確認します。
その場合、`new_line`呼び出しは、行を折り返すために使用されます。
次に、それはバッファの現在の位置に新しい`ScreenChar`を書き込みます。
最後に、現在の列の位置を進めます。

全体の文字列を出力するために、それらを`bytes`に変換して、それらを1つずつ出力します。

```rust
// in src/vga_buffer.rs

impl Write {
    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 出力可能なASCIIバイトか改行
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 出力可能なASCIIの範囲でない部分
                _ => self.write_byte(0xfe);
            }
        }
    }
}
```

VGAテキスト・バッファはASCIIと[コード・ページ437]()の追加バイトのみをサポートします。
Rustの文字f列はデフォルトで[`UTF-8`](https://www.fileformat.info/info/unicode/utf8.htm)であるため、それらはVGAテキスト・バッファによってサポートされていないバイトが含まれているかもしれません。
出力可能なASCIIバイト（改行、スペース文字と`~`文字の間の任意の文字）と出力できないバイトを区別するために`match`を使用しています。
出力できないバイトのために、VGAハードウェアにおいて16進コード`0xfe`を持つ`■`文字を出力します。

#### やってみよう！

スクリーンにいくつかの文字を書き込みするために、一時的な関数を作成できます。

```rust
// in src/vga_buffer.rs
pub fn print_something() {
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        // BufferをVGAバッファを埋め尽くすように展開される
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
    writer.write_byte(b'H');
    writer.write_string("ello ");
    writer.write_string("Wörld!");
}
```

最初に、`0xb8000`にあるVGAバッファを指し示す新しいライターを作成します。
この構文は少し奇妙に見えるかもしれません。
最初に、整数`0xb8000`を可変な[生ポインタ](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#dereferencing-a-raw-pointer)にキャストしています。
そして、それをその参照外しを介して（`*`を介して）可変参照に変換して、すぐにそれを（`&mut`を介して）再度借用しています。
コンパイラは生ポインタが有効か保証することができないため、この変換は[`unsafeブロック`](unsafe block)を必要としています。

次に`b'H'`を書き込みます。
前置後の`b`は、ASCII文字を表現する`バイト・リテラル`を作成します。
文字列`"ello"`と`"Wörld!"`を書き込みにより、`write_string`メソッドと出力できない文字の取り扱いをテストしています。
出力を確認するために、`_start`関数から`pring_something`関数を呼び出す必要があります。

```rust
// in src/main.rs

#[no_mangle]
pub extern "C" fn _start() -> ! {
    vga_buffer::print_something();

    loop {}
}
```

現時点でプロジェクトを実行したとき、`Hello W■■rld!`が黄色で左下の角に出力されます。

![Hello W■■rld!](https://os.phil-opp.com/vga-text-mode/vga-hello.png)

`ö`が2つの`■`文字で出力されることに注意してください。
[UTF-8](https://www.fileformat.info/info/unicode/utf8.htm)において、`ö`は2バイトで表現されており、その両方は出力可能なASCIIの範囲外です。
事実、UTF-8の基本的な特性で、複数バイトの個々のバイトの値は、決して妥当なASCIIではありません。

### 揮発性

我々のメッセージが正確に出力されたことを確認しました。
しかし、それはより貪欲に最適化する将来のRustコンパイラで機能しないかもしれません。

問題は、単に`Buffer`に書き込んだだけで、再びそれから読み込むことはありません。
コンパイラは、VGAバッファ・メモリ（代わりに通常のRAM）にアクセスすることを本当に知っておらず、スクリーンにいくつかの文字が現れるという副作用について知っていません。
よって、これらの書き込みは不必要で省略できると判断する場合があります。
この誤った最適化を避けるために、[*volatile*](https://en.wikipedia.org/wiki/Volatile_(computer_programming))としてこれらの書き込みを指定する必要があります。
これは、その書き込みは副作用を持っており、最適化から除外すべきであることをコンパイラに指示します。

> `volatile`（揮発性）とは、コードの制御外で、値が時間の経過とともに変化する傾向があることを意味しています。

VGAバッファへの揮発性書き込みを使用するために、[volatile](https://docs.rs/volatile)ライブラリを使用します。
このクレート（Rustの世界において、パッケージの呼び方です）は、`read`と`write`メソッドを備えた`Volatile`ラッパー型を提供します。
これらのメソッドは、内部的にコア・ライブラリの[read_volatile](https://doc.rust-lang.org/nightly/core/ptr/fn.read_volatile.html)と[write_volatile](https://doc.rust-lang.org/nightly/core/ptr/fn.write_volatile.html)関数を使用するので、読み込み／書き込みは最適化されないことを保証します。

`Cargo.toml`の`dependencies`セクションに、`volatile`クレートへの依存関係を追加できます。

```toml
# in Cargo.toml

[dependencies]
volatile = "0.2.6"
```

バーション`0.2.6`の`volatile`を指定することを確実にしてください。
そのクレートの新しいバージョンは、この投稿と互換性がありません。
`0.2.6`は[セマンティック](https://semver.org/)バージョン番号です。
詳細は、cargoドキュメントの[依存関係の指定](https://doc.crates.io/specifying-dependencies.html)ガイドを参照してください。

それを使用して、VGAバッファへの書き込みを揮発性にしましょう。
次のように`Buffer`型を更新します。

```rust
// in src/vga_buffer.rs

use volatile::Volatile;

struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}
```

`ScreenChar`の代わりに、現在`Volatile<ScreenChar>`を仕様しています。
（`Volatile`型は[ジェネリック]()で（ほとんど）任意の方を｀ラップできます。）
これは、不注意で「普通に」それに書き込みできないことを保証します。
代わりに、現在`write`メソッドを使用する必要があります。

これは、`Writer::write_byte`メソッドを更新する必要があることを意味します。

```rust
// in src/vga_buffer.rs

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                ...
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii_character: byte,
                    color_code,
                });
                ...
            }
        }
    }
}
```

`=`を使用した典型的な割り当ての代わりに、現在`write`メソッドを使用しえいます。
現在、コンパイラがこの書き込みを最適化しないことを保証します。

### 書式マクロ

Rustの書式マクロをサポートすることは素晴らしいことです。
そうすれば、整数や浮動小数点数のような様々な型の出力を簡単にできるようになります。
それらをサポートするために、[`core::fmt::Write`](https://doc.rust-lang.org/nightly/core/fmt/trait.Write.html)トレイトを実装する必要があります。
このトレイトの唯一の必須メソッドは`write_str`で、それはとても我々が実装した`write_string`メソッドに見た目が似ていて、`fmt::Result`が戻り値の型です。

```rust
// in src/vga_buffer.rs

use core::fmt;

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);

        Ok(())
    }
}
```

`Ok(())`は`()`型を含んでいる`Result::Ok`です。

現在、Rustのビルトイン`write!`／`writeln!`書式マクロを使用することができます。

```rust
// insrc/vga_buffer.rs

pub fn print_something() {
    use core::fmt::Write;
    let mut writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
    writer.write_byte(b'H');
    writer.write_string("ello! ");
    write!(writer, "The number s are {} and {}", 42, 1.0 / 3.0).unwrap();
}
```

現在、スクリーンの下に`Hello! The numbers are 42 and 0.3333333333333333`が表示されるはずです。
`write!`呼び出しは、使用されない場合に警告を報告する`Result`を返却するので、もしエラーが発生したっ場合にパニックする[`unwrap`](https://doc.rust-lang.org/core/result/enum.Result.html#method.unwrap)関数を呼び出します。
VGAバッファへの書き込みは決して失敗しないので、これは我々のケースで問題になりません。

### 改行

たった今、単に改行とこれ以上業に収まらない文字を単に無視しました。
代わりに、すべての文字を1行上に移動（一番上の行は削除されます）して、再度最終行の始めから開始することを望んでいます。
これをするために、`Writer`の`new_line`メソッドの実装を追加します。

```rust
// in src/vga_buffer.rs

impl Writer {
    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        unimplemented!();
    }
}
```

すべてのスクリーンの文字を順に繰り返して、それそれの文字を1行上に移動します。
範囲表記（`..`）の上限は排他的であることに注意してください。
また、0番目の行はスクリーンの外にシフトされるので、0番目の行（最初の範囲は`1`から始まります）は無視しています。

`newline`コードを仕上げるために、`clear_row`メソッドを追加します。

```rust
// in src/vga_buffer.rs

impl Write {
    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}
```

このメソッドはスペース文字でその行の文字をすべて上書きすることで、行をクリアします。

## グローバル・インターフェース

`Writer`インスタンスをあちこちに持ち出すことなしに、他のモジュールからインターフェースとして使用されるグローバル・ライターを提供するために、静的な`WRITER`を作成することを試みます。

```rust
// in src/vga_buffer.rs

pub static WRITER: Writer = Writer {
    column_position: 0,
    color_code: ColorCode::new(Color::Yellow, Color::Black),
    buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
};
```

しかしながら、今そのコンパイルを試みる場合、以下のエラーが発生します。

```text
error[E0015]: calls in statics are limited to constant functions, tuple structs and tuple variants
 --> src/vga_buffer.rs:7:17
  |
7 |     color_code: ColorCode::new(Color::Yellow, Color::Black),
  |                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

error[E0396]: raw pointers cannot be dereferenced in statics
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ dereference of raw pointer in constant

error[E0017]: references in statics may only refer to immutable values
 --> src/vga_buffer.rs:8:22
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values

error[E0017]: references in statics may only refer to immutable values
 --> src/vga_buffer.rs:8:13
  |
8 |     buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ statics require immutable values
```

ここで何が発生したのかを理解するためには、実行時に初期化される通常の変数とは対照的に、静的変数はコンパイル時に初期化されることを理解する必要があります。
実行時にそのような初期化式を評価するRustコンパイラのコンポーネントは、「[定数評価器](https://rustc-dev-guide.rust-lang.org/const-eval.html)」と呼ばれています。
その昨日は未だ限定的ですが、それを拡大するための作業が進行中で、「[Allow panicking in constants](https://github.com/rust-lang/rfcs/pull/2345)」RFCにあります。

`ColorCode::new`の問題は、[`const`関数]()を使用することで解決されるでしょうが、この基本的な問題は、Rustの定数評価器がコンパイル時に生ポインタを参照に変換することができないことです。
おそらく、それはいつか出来るでしょうが、まだなので、他の解決を探す必要があります。

### 静的化遅延

Rustにおいて、非定数関数を使用した静的な1度の初期化は、一般的な問題です。
幸運にも、[lazy_static](https://docs.rs/lazy_static/1.0.1/lazy_static/)と名前が付けられたクレートに、良い解決方法がすでに存在します。
このクレートは、`静的化`遅延は、シア所にアクセスされたときにそれ自身で初期化します。
従って、初期化は実行時に発生するので、任意な複雑な初期化コードが可能です。

我々のプロジェクトに`lazy_static`クレートを追加しましょう。

```toml
// in Cargo.toml

[dependencies.lazy_static]
version = "1.0"
features = ["spin_no_std"]
```

標準ライブラリとリンクしないので、`spin_no_std`フィーチャが必要です。

`lazy_static`を使用して、問題のない静的な`WRITER`を定義できます。

```rust
// in src/vga_buffer.rs

use core::fmt::{self, Write};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref WRITER: Writer = Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    };
}
```

しかしながら、この`WRITER`は、不変なのでかなり役に立ちません。
これは、それに何も書き込むことができないことを意味します（すべての書き込みメソッドは`&mut self`を受け取るため）。
1つの可能性のある解決は、[`可変な静的変数`](https://doc.rust-lang.org/book/ch19-01-unsafe-rust.html#accessing-or-modifying-a-mutable-static-variable)を使用することです。
ただし、簡単にデータの競合やその他の悪いことが発生する可能性があるため、それへのすべての読み込みと書き込みは不安定です。
`static mut`の仕様は勧められません。
それを[削除する](https://internals.rust-lang.org/t/pre-rfc-remove-static-mut/1437)提案さえありました。
しかし、代替手段は何でしょうか？
[内部可変性](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html)を提供する[RefCell](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#keeping-track-of-borrows-at-runtime-with-refcellt)や[UnsafeCell](https://doc.rust-lang.org/nightly/core/cell/struct.UnsafeCell.html)のようなセル型で不変静的変数を使用することを試みることができます。
しかし、これらの型は（良い理由で）[Sync](https://doc.rust-lang.org/nightly/core/marker/trait.Sync.html)ではないため、静的変数でそれらを使用することができません。

> `Sync`トレイトは、複数のスレッドからのアクセスを許可するマーカー・トレイトです。

### スピンロック

同期した内部可変性を得るために、標準ライブラリのユーザーは[Mutex](https://doc.rust-lang.org/nightly/std/sync/struct.Mutex.html)を使用できます。
それは、リソースが既にロックされているとき、スレッドをブロックすることによって、相互排除を提供します。
しかし、我々の基本的なカーネルは、ブロッキング・サポートとスレッドの概念さえ持っていないので、それを利用することができません。
しかし、コンピューター・サイエンスには、オペレーティング・システムの機能を必須としない、本当に基本的な種類のミューテックスである[スピンロック](https://en.wikipedia.org/wiki/Spinlock)があります。
ブロッキングする代わりに、スレッドはきついループないで何度もロックすることを試みるため、再びミューテックスが開放されるまでCPU時間を費やします。

スピニング・ミューテックスを使用するために、依存関係として[spinクレート](https://crates.io/crates/spin)を追加できます。

```toml
// in Cargo.toml
[dependencies]
spin = "0.5.2"
```

次に、我々の静的な`WRITER`に安全な[内部可変性](https://doc.rust-lang.org/book/ch15-05-interior-mutability.html)を追加するために、スピニング・ミューテックスを使用できます。

```rust
// in src/vga_buffer.rs

use spin::Mutex;

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        color_code: ColorCode::new(Color::Yellow, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
    });
}
```

現在、`print_something`関数を削除して、`_start`関数から直接出力できます。

```rust
// in src/main.rs

#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    use core::fmt::Write;
    vga_buffer::WRITER.lock().write_str("Hello again").unwrap();
    write!(vga_buffer::WRITER.lock(), ", some number: {} {}", 42, 1337).unwrap();

    loop {}
}
```

`fmt::Write`トレイトの関数を使用できるようにするために、`fmt::Write`トレイトをインポートする必要があります。

### 安全性

`0xb8000`を指し示す`Buffer`の参照を作成するために、コード内に一つのアンセーフ・ブロックのみがあることに注意してください。
その後、すべての操作は安全です。
Rustはデフォルトで配列にアクセスするための境界チェックを使用するので、不注意でバッファの外に書き込むことができません。
従って、型システム内に必須条件をエンコードして、外部に安全なインターフェースを提供することができました。

### printlnマクロ

現在、グローバルなライターを持っているので、コードベースないのどこからでも使用することができる`println`マクロを追加できます。
Rustの[マクロ構文](https://doc.rust-lang.org/nightly/book/ch19-06-macros.html#declarative-macros-with-macro_rules-for-general-metaprogramming)は少し変わっているので、最初からマクロを記述することを試みない予定です。
代わりに、標準ライブラリの[`println!`マクロ](https://doc.rust-lang.org/nightly/std/macro.println!.html)のソースを見ます。

```rust
#[macro_export]
macro_rule! println {
    () => (print!("\n"));
    ($($arg:tt)*) => (print!("{}\n", format_args!($($arg)*)));
}
```

マクロは、`match`アームに似た、1つまたは複数のルールを介して定義されます。
`println`マクロは2つのルールを持っています。
最初のルールは、例えば`println~()`のように引数なしで呼び出された場合で、それは`print!("\n")`と展開され、単に改行を出力します。
2番目のルールは、`println!("Hello")`または`println!("Number: {}", 4)`のように引数を伴って呼び出された場合です。
それも`print!`マクロの呼び出しに展開され、すべての引数と末尾に改行追加の改行`\n`を渡します。

`#[marco_export]`属性は、クレート全体（単にそれが定義されたモジュールだけでなく）と外部クレートでマクロを利用できるようにします。
それはクレートのルートにマクロを配置するため、それは`std::macros::println`の代わりに`use std::println`を通じてマクロをインポートしなければならないことを意味します。

> 標準ライブラリの`println`マクロは、`std/macros.rs`ファイルに実装されています。

[`print!`マクロ](https://doc.rust-lang.org/nightly/std/macro.print!.html)は、次のように定義されています。

```rust
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::io::_print(format_args!($($args)*)));
}
```

そのマクロは`io`モジュール内の[`_print`関数](https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698)の呼び出しに展開されます。
[`$crate`変数](https://doc.rust-lang.org/1.30.0/book/first-edition/macros.html#the-variable-crate)は、他のクレートでマクロが使用されたときに`std`に展開することで、マクロが`std`クレートの外からでも動作することを保証します。

[`format_args`マクロ](https://doc.rust-lang.org/nightly/std/macro.format_args.html)は、渡された引数から[fmt::Arguments](https://doc.rust-lang.org/nightly/core/fmt/struct.Arguments.html)型を構築して、それを`_print`に渡します。
libstdの[`_print`関数](https://github.com/rust-lang/rust/blob/29f5c699b11a6a148f097f82eaa05202f8799bbc/src/libstd/io/stdio.rs#L698)は`print_to`を呼び出し、それはさまざまな`Stdout`デバイスをサポートしているため、より複雑です。
我々はVGAバッファに出力したいだけなので、そのような複雑性は必要ありません。

VGAバッファに出力するために、単に`println!`と`print!`マクロをコピーしますが、独自の`_print`関数を使用するためにそれらを変更します。

```rust
// in src/vga_buffer.rs

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
```

オリジナルの`println`定義から変更した1つは、`print!`マクロの呼び出しにも`$crate`を前に付けたことです。
これは、もし`println`のみを使用したい場合でも、`print!`マクロをインポートする必要がないことを保証します。

標準ライブラリのように、我々のクレート内のどこでも両方のマクロを利用できるようにするために、`#[macro_export]`属性を追加します。
これは、クレートのルート名前空間内にマクロを配置するため、それらを`use crate::vga_buffer::println`によってインポートすることは動作しません。
`use crate::println`でする必要があります。

`_print`関数は静的`WRITER`をロックして、その`write_fmt`メソッドを呼び出します。
このメソッドは`Write`トレイトに由来するため、それをインポートする必要があります。
最後の追加的な`unwrap()`は、もし出力が失敗するとパニックします。
しかし、`write_str`は常に`Ok`を返却するので、それが発生するべきではありません。

マクロは外部モジュールから呼び出すことが出来る必要があるため、関数は公開される必要があります。
しかしながら、これは非公開の実装の詳細と考えるため、ドキュメントの生成からそれを隠すために、[`doc(hidden)`属性](https://doc.rust-lang.org/nightly/rustdoc/write-documentation/the-doc-attribute.html#hidden)を追加しています。

### `println`を使用したHello World

`_start`関数内で`println`を使用できます。

```rust
// in src/main.rs

#[no_mangle] // この関数の名前をマングルしない
pub extern "C" fn _start() -> ! {
    // この関数はエントリポイントであるため、 リンカはデフォルトで`_start`という名前の関数を探す
    println!("Hello World{}", "!");

    loop {}
}
```

マクロは既にルート名前空間に存在するため、main関数内でマクロをインポートする必要がないことに注意してください。

予想されたように、現在スクリーン上に*"Hello Workd"*が見えます。

![Hello World!](https://os.phil-opp.com/vga-text-mode/vga-hello-world.png)

### パニック・メッセージの表示

現在、`println`マクロを持っているので、我々のパニック関数内で、パニック・メッセージとパニックが発生した場所を出力するために、それを使用することができます。

```rust
/// This function is called on panic.
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}
```

現在、`_start`関数に`panic!("SOme panic message");`を挿入したとき、以下の出力が得られます。

![Insert panic in _start function](https://os.phil-opp.com/vga-text-mode/vga-panic.png)

## まとめ

この投稿において、VGAテキストバッファの構造と、どのようにアドレス`0xb8000`へのメモリ・マッピングを通じてどのように書き込むことが出来るかを学びました。
このメモリ・マップド・バッファへの危険な書き込みを閉じ込めたRustモジュールを作成して、安全で便利な外部とのインターフェースを提供します。

cargoのおかげdえ、サード・パーティ・ライブラリの依存関係を追加することが簡単であることがわかりました。
追加シア2つの依存関係、`lazy_static`と`spin`は、OS開発にとても役立ち、今後の投稿においてさらに多くの場所で使用する予定です。

## 次は何ですか？

次の投稿は、Rustがビルトインする単体テスト・フレームワークを準備する方法を説明します。
次に、この投稿のVGAバッファ・モジュールのための基本的な単体テストを幾つか作成する予定です。
