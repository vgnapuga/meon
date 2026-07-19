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
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md)
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
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
    ...
}
```

Все спаны — `u32` байтовые смещения, 8 байт на спан. Доступ к любому виду элементов — O(1). Итерация всех жирных спанов — единственный прямой проход по непрерывному массиву. Префетчер CPU доволен.

---

## Спаны — zero-copy доступ в источник

Каждый элемент представлен как `Span { start: u32, end: u32 }` — полуоткрытый байтовый диапазон `[start, end)` в оригинальный срез источника. Ничего не копируется. Ничего не декодируется пока вы сами не попросите.

```rust
let src = "**жирный** и *курсив*\n".as_bytes();
let c = MarkdownParser::parse(src);

// Разрешаем спан в строковый срез — zero copy, borrow из источника.
// Возвращает `None` при невалидном UTF-8 вместо паники.
let text: &str = c.str(c.bolds[0]).unwrap();
assert_eq!(text, "жирный");

// Или работаем с сырыми байтами, без проверки UTF-8
let bytes: &[u8] = c.bytes(c.italics[0]);

// Каждое поле также получает сгенерированный аксессор `_clean` (разделители
// вырезаны) и `_raw` (разделители включены) — zero-copy итераторы байтовых срезов.
let raw: &[u8] = c.bolds_raw().next().unwrap();
assert_eq!(raw, "**жирный**".as_bytes());
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

Standalone-итераторы быстрее полного парса когда нужен только один вид элементов — они пропускают всё межэлементное bookkeeping. Вложенность блоков одного типа (`> >` открывает два фрейма цитаты) совпадает с полным парсом и ограничена `max_nest` грамматики. Оставшийся компромисс — непрозрачность: маркер жирного текста внутри блока с кодом будет найден `find_bolds`, но подавлен полным парсером. Это расхождение намеренно, задокументировано — и закрываемо:

```rust
// Постройте карту непрозрачных регионов один раз (ограждённые блоки,
// код-спаны, автоссылки) и гоняйте по ней любое число контекстных файндеров.
let ctx = MarkdownParser::context(src);
for span in MarkdownParser::find_context_bolds(src, &ctx) {
    // маркеры жирного внутри код-спанов и ограждённых блоков пропущены,
    // как в полном парсе
}
```

Каждое правило, не являющееся само непрозрачным, получает вариант `find_context_*`; карта строится одним потоковым проходом и разделяется между всеми.

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
// MyFormatParser::context(src) -> ParseContext
// MyFormatParser::find_context_bolds(src, &ctx) -> impl Iterator<Item = Span>
// MyFormatParser::find_headings(src) -> impl Iterator<Item = (Heading, Span)>
// MyFormatContent::bolds_clean() -> impl Iterator<Item = &[u8]>
// MyFormatContent::bolds_raw()   -> impl Iterator<Item = &[u8]>
// ... и многое другое
```

Всё — контент-структура, метод parse, все find_*-итераторы, все аксессоры — генерируется во время компиляции. Никакого рантайм-диспатча, никаких vtable, никакого интерпретатора грамматики.

---

## Структура репозитория

```
meon/                 ← корень воркспейса (этот файл)
├── meon/             ← движок парсинга + рантайм макросы
├── meon-macros/      ← прок-макрос define_parser!
├── meon-md/          ← грамматика Markdown построенная на meon
├── meon-json/        ← грамматика-ридер JSON построенная на meon
├── benches/          ← бенчмарки criterion
└── fuzz/             ← cargo-fuzz харнес
```

`meon-md` — конкретная грамматика которая парсит полезное подмножество Markdown. Она демонстрирует что движок покрывает реальную сложность, и служит бенчмарком и fuzz-целью проекта.

`meon-json` — вторая референс-грамматика, плоский span-based JSON-ридер. Она показывает, что движок не привязан к Markdown: структурно противоположный формат — глубокая вложенность, контейнеры, пары `key: value`, переносы строк как обычное содержимое — выводится из тех же примитивов `define_parser!`, выдавая один плоский `Vec` на каждый вид элементов (объекты, массивы, строки, члены) вместо дерева.

---

## Feature flags

| Feature  | Эффект                                                     |
|----------|------------------------------------------------------------|
| `avx2`   | 32-байтовые SIMD-лейны (требует nightly + AVX2 процессор)  |
| `avx512` | 64-байтовые SIMD-лейны (включает `avx2`)                   |

Без обоих флагов крейт компилируется на стабильном Rust.

---

## Лицензия

`meon` доступен под
[***MIT***](./LICENSE-MIT) *ИЛИ* [***APACHE-2.0***](./LICENSE-APACHE) лицензией.
