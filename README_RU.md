# meon

[**EN**](https://github.com/vgnapuga/meon/blob/main/README.md) | RU

> Декларативный плоский движок парсинга текстовых форматов.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Что такое meon

Большинство парсеров строят дерево. `meon`([wiki](https://en.wikipedia.org/wiki/Meon_(philosophy))) строит таблицу.

Вы описываете грамматику один раз через `define_parser!` и получаете контент-структуру, в которой каждый тип элементов живёт в своём плоском `Vec`. Заголовки в одном векторе, жирный текст в другом, ссылки в третьем. Никакого обхода дерева, никакого давления на аллокатор от узловых объектов, никакого виртуального диспатча. Просто непрерывные массивы пар `u32`-смещений, которые итерируются на нативной скорости.

---

## Плоский вывод — дружелюбный к железу по дизайну

Типичный парсер отдаёт гетерогенное AST. Чтобы найти все жирные спаны, нужно обойти дерево, совпасть с типами узлов и собрать нужное. Cache miss'ы накапливаются при прыжках между связанными указателями узлами разного размера.

`meon` переворачивает это с ног на голову. Вывод — struct-of-arrays:

```
MarkdownContent {
    source:         &[u8]               ← оригинальные байты, borrowed
    texts:          Vec<Span>           ← все прогоны простого текста
    bolds:          Vec<Span>           ← все жирные спаны
    italics:        Vec<Span>           ← все курсивные спаны
    codes:          Vec<Span>           ← все спаны инлайн-кода
    links:          Vec<Link>           ← все ссылки и изображения
    headings:       Vec<(Heading, Span)>
    fenced_codes:   Vec<Span>
    bullet_items:   Vec<(BulletItem, Span)>
    …
}
```

Все спаны — `u32` байтовые смещения, 8 байт на спан. Доступ к любому виду элементов — O(1). Итерация всех жирных спанов — единственный прямой проход по непрерывному массиву. Префетчер CPU доволен.

---

## Спаны — zero-copy доступ в источник

Каждый элемент представлен как `Span { start: u32, end: u32 }` — полуоткрытый байтовый диапазон `[start, end)` в оригинальный срез источника. Ничего не копируется. Ничего не декодируется пока вы сами не попросите.

```rust
let src = b"**жирный** и *курсив*\n";
let c = MarkdownParser::parse(src);

// Разрешаем спан в строковый срез — zero copy, borrow из источника
let text: &str = c.str(c.bolds[0]).unwrap();
assert_eq!(text, "жирный");

// Или работаем с сырыми байтами
let bytes: &[u8] = c.bytes(c.italics[0]);
```

Контент-структура заимствует источник на всё своё время жизни. Когда структура дропается — источник освобождается. Никаких промежуточных представлений не остаётся.

---

## Контекст-свободная экстракция — парсим один тип без парсинга всего

Каждое правило грамматики генерирует standalone-итератор `find_*`. Он сканирует сырой источник только ради одного вида элементов, без знания об окружающих элементах, активных блоках или состоянии параграфов.

```rust
// Полный парс — все виды элементов за один проход
let content = MarkdownParser::parse(src);

// Standalone — только жирные спаны, больше ничего не вычисляется
for span in MarkdownParser::find_bolds(src) {
    println!("{}", std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

// Заголовки с метаданными — уровень и спан содержимого
for (heading, span) in MarkdownParser::find_headings(src) {
    println!("h{}: {}", heading.level, std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

// Ссылки и изображения — структурный тип, два спана внутри
for link in MarkdownParser::find_links(src) {
    let text = std::str::from_utf8(&src[link.text.start as usize..link.text.end as usize]).unwrap();
    let url  = std::str::from_utf8(&src[link.url.start  as usize..link.url.end  as usize]).unwrap();
    println!("[{}]({})  image={}", text, url, link.is_image);
}
```

Standalone-итераторы быстрее полного парса когда нужен только один вид элементов — они пропускают всё межэлементное bookkeeping. Компромисс в том, что они работают без контекста: маркер жирного текста внутри блока с кодом будет найден `find_bolds`, но подавлен полным парсером. Это расхождение намеренно и задокументировано.

---

## Декларативная грамматика — один вызов, полный парсер

У движка нет встроенных знаний ни о каком текстовом формате. Вы описываете свой формат как грамматику, и движок компилирует её в парсер во время сборки:

```rust
use meon::define_parser;

define_parser!(MyFormat {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline {
        on_trigger(b'*') {
            symmetric b'*' {
                parse_inside = true;
                balanced     = false;
                1 => italics [40],
                2 => bolds   [40],
            }
        }
        fallback => texts [10];
    }
    lines {
        line(b'#', max = 6) |n|:
            Heading { level: NonZeroU8::new(n).unwrap_or(NonZeroU8::MIN) }
            => headings [200];
    }
    blocks {
        block_simple {
            fence(b'`', min = 3) => fenced_codes [400];
        }
        fallback => paragraphs [80];
    }
});

// Сгенерировано:
// MyFormatParser::parse(src) -> MyFormatContent<'_>
// MyFormatParser::find_bolds(src) -> impl Iterator<Item = Span>
// MyFormatParser::find_headings(src) -> impl Iterator<Item = (Heading, Span)>
// MyFormatContent::bolds_clean() -> impl Iterator<Item = &[u8]>
// MyFormatContent::bolds_raw()   -> impl Iterator<Item = &[u8]>
// … и многое другое
```

Всё — контент-структура, метод parse, все find_*-итераторы, все аксессоры — генерируется во время компиляции. Никакого рантайм-диспатча, никаких vtable, никакого интерпретатора грамматики.

---

## Структура репозитория

```
meon/                 ← корень воркспейса (этот файл)
├── meon/             ← движок парсинга + рантайм макросы
├── meon-macros/      ← прок-макрос define_parser!
├── meon-md/          ← грамматика Markdown построенная на meon
├── benches/          ← бенчмарки criterion
└── fuzz/             ← cargo-fuzz харнес
```

`meon-md` — конкретная грамматика которая парсит полезное подмножество Markdown. Она демонстрирует что движок покрывает реальную сложность, и служит бенчмарком и fuzz-целью проекта.

---

## Feature flags

| Feature  | Эффект                                                   |
|----------|----------------------------------------------------------|
| `avx2`   | 32-байтовые SIMD-лейны (требует nightly + AVX2 процессор)|
| `avx512` | 64-байтовые SIMD-лейны (включает `avx2`)                 |

Без обоих флагов крейт компилируется на стабильном Rust.

---

## Лицензия

`meon` доступен под лицензией
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE).

Если условия AGPL-3.0 несовместимы с вашим сценарием использования, доступна коммерческая лицензия — см. [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md).

Внося вклад в проект, вы соглашаетесь с [***Соглашением о лицензировании контрибуций (CLA)***](https://github.com/vgnapuga/meon/blob/main/CLA.md).
