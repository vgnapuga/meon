# meon-md

[**EN**](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md) | RU

Быстрый плоский парсер подмножества Markdown, построенный на движке
[`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).

`meon-md` — это одновременно готовый к использованию Markdown-парсер и
эталонная грамматика, демонстрирующая что `meon` способен выразить в одном
вызове `define_parser!`.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**    <--
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

## Быстрый старт

```toml
[dependencies]
meon-md = "0.3"
```

```rust
use meon_md::MarkdownParser;

let src = b"# Hello\n**world** and *italic* with `code`\n";
let c = MarkdownParser::parse(src);

// Доступ по виду элементов — O(1) на вид
println!("заголовков:  {}", c.headings.len());
println!("жирных:      {}", c.bolds.len());
println!("курсивных:   {}", c.italics.len());

// Разрешаем спан в строковый срез
if let Some((_, span)) = c.headings.first() {
    println!("текст заголовка: {}", c.str(*span).unwrap());
}
```

---

## Поддерживаемые элементы

### Инлайн

| Элемент         | Синтаксис                   | Поле           | Тип           |
|-----------------|-----------------------------|----------------|---------------|
| Простой текст   | любые несовпавшие байты     | `texts`        | `Vec<Span>`   |
| Жирный          | `**текст**`                 | `bolds`        | `Vec<Span>`   |
| Курсив          | `*текст*`                   | `italics`      | `Vec<Span>`   |
| Жирный курсив   | `***текст***`               | `bold_italics` | `Vec<Span>`   |
| Инлайн-код      | `` `код` ``                 | `codes`        | `Vec<Span>`   |
| Ссылка          | `[текст](url)`              | `links`        | `Vec<Link>`   |
| Изображение     | `![alt](url)`               | `links`        | `Vec<Link>`   |
| Автоссылка      | `<url>`                     | `autolinks`    | `Vec<Span>`   |
| Жёсткий перенос | `\` или `··` в конце строки | `hard_breaks`  | `Vec<Span>`   |

### Строчные

| Элемент             | Синтаксис           | Поле              | Тип                          |
|---------------------|---------------------|-------------------|------------------------------|
| Заголовок           | `# ... ######`      | `headings`        | `Vec<(Heading, Span)>`       |
| Тематический разрыв | `---`, `***`, `___` | `thematic_breaks` | `Vec<(ThematicBreak, Span)>` |

### Блочные

| Элемент              | Синтаксис          | Поле            | Тип                          |
|----------------------|--------------------|-----------------|------------------------------|
| Блок кода            | ` ``` ... ``` `    | `fenced_codes`  | `Vec<Span>`                  |
| Цитата               | `> ...`            | `blockquotes`   | `Vec<Span>`                  |
| Элемент списка       | `- / * / +`        | `bullet_items`  | `Vec<(BulletItem, Span)>`    |
| Нумерованный элемент | `1. / 1)`          | `ordered_items` | `Vec<(OrderedItem, Span)>`   |
| Параграф             | fallback           | `paragraphs`    | `Vec<Span>`                  |

---

## Выходные типы

```rust
// Span — полуоткрытый байтовый диапазон [start, end) в срез источника
pub struct Span { pub start: u32, pub end: u32 }

// Link — несёт спаны текста и url плюс флаг изображения
pub struct Link {
    pub is_image: bool,
    pub text: Span,
    pub url:  Span,
}

// Heading — уровень вложенности 1–6
pub struct Heading { pub level: NonZeroU8 }

// ThematicBreak — ASCII байт разделителя (b'-', b'*' или b'_')
pub struct ThematicBreak { pub kind: u8 }

// BulletItem — ASCII байт маркера (b'-', b'*' или b'+')
pub struct BulletItem { pub kind: u8 }

// OrderedItem — разобранное число и байт разделителя (b'.' или b')')
pub struct OrderedItem { pub kind: u8, pub num: u32 }
```

---

## Работа со спанами

Контент-структура заимствует оригинальный источник. Используйте встроенные
хелперы для разрешения спанов:

```rust
let src = b"**жирный** и *курсив*\n";
let c = MarkdownParser::parse(src);

// str() возвращает None при невалидном UTF-8 вместо паники
if let Some(text) = c.str(c.bolds[0]) {
    println!("жирный: {text}");   // → "жирный"
}

// bytes() для сырого байтового доступа
let raw: &[u8] = c.bytes(c.italics[0]);

// _clean() итератор — внутреннее содержимое без байт разделителей
for text in c.bolds_clean() {
    println!("{}", std::str::from_utf8(text).unwrap());
}

// _raw() итератор — полный срез включая байты разделителей
for raw in c.bolds_raw() {
    println!("{}", std::str::from_utf8(raw).unwrap());  // → "**жирный**"
}
```

---

## Standalone-итераторы

Каждый вид элементов имеет метод `find_*` который сканирует источник без
полного парса. Используйте его когда нужен только один вид элементов из
большого документа:

```rust
use meon_md::MarkdownParser;

let src = long_document.as_bytes();

// ~2–5× быстрее полного парса для одного вида элементов
for span in MarkdownParser::find_bolds(src) {
    println!("{}", std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}

for link in MarkdownParser::find_links(src) {
    // link.text, link.url, link.is_image
}

for (heading, span) in MarkdownParser::find_headings(src) {
    println!("h{}: {}", heading.level, std::str::from_utf8(&src[span.start as usize..span.end as usize]).unwrap());
}
```

Standalone-итераторы работают без межэлементного контекста: они могут
возвращать спаны которые полный парсер подавил бы (например, маркеры жирного
текста внутри блока с кодом). Вложенность цитат, однако, совпадает с полным
парсом — `find_blockquotes` видит `> >` как два вложенных спана, ограниченных
грамматическим `max_nest`.

Чтобы закрыть разрыв по непрозрачности, постройте карту контекста один раз и
используйте варианты `find_context_*`:

```rust
let ctx = MarkdownParser::context(src);
// Маркеры жирного внутри кодовых спанов, автоссылок и блоков с кодом пропускаются:
for span in MarkdownParser::find_context_bolds(src, &ctx) { /* ... */ }
```

Такой есть у каждого непрозрачного вида элемента (`find_context_bolds`,
`find_context_headings`, `find_context_blockquotes`, ...); кодовые спаны,
автоссылки и блоки с кодом — источники контекста и сохраняют только свой
контекст-свободный `find_*`. Подробнее —
[`ARCHITECTURE_RU.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE_RU.md#12-standalone-итераторы) - *GitHub*.

---

## Feature flags

Наследуются от `meon`:

| Feature  | Эффект                                                      |
|----------|-------------------------------------------------------------|
| `avx2`   | 32-байтовый SIMD-поиск (требует nightly + AVX2 процессор)   |
| `avx512` | 64-байтовый SIMD-поиск (включает `avx2`)                    |

```toml
[dependencies]
meon-md = { version = "0.1", features = ["avx2"] }
```

---

## Вложенность

Эта грамматика устанавливает `max_nest = 4`. Два независимых механизма
используют эту настройку:

- **Цитаты и ограждения** вкладываются до 4 уровней. `> > текст` открывает
  два различных, корректно ограниченных спана `blockquotes`, а не один
  свёрнутый спан, в который утекает внутренний маркер; блок кода открытый
  на строке продолжения внутри цитаты остаётся ограничен своим собственным
  спаном.
- **Жирный и курсив** вкладываются до 4 уровней. `**жирный *курсив* всё ещё
  жирный**` корректно разрешает и внешний жирный, и внутренний курсив,
  вместо того чтобы внутренний разделитель молча перезатирал внешний.

```rust
let src = "> > вложенная цитата с **жирным *курсивным* текстом**\n".as_bytes();
let c = MarkdownParser::parse(src);
assert_eq!(c.blockquotes.len(), 2);
assert_eq!(c.bolds.len(), 1);
assert_eq!(c.italics.len(), 1);
```

Ссылки, изображения и автоссылки остаются невкладываемыми по дизайну —
`[a [b] c](url)` не вкладывает свои собственные скобки.

---

## Известные ограничения

Это **демонстрационная грамматика**, а не реализация совместимая с CommonMark.

- Акцентирование охватывающее несколько строк не обнаруживается.
- Приоритет акцентирования не соблюдается — побеждает порядок объявления.
- Ссылочный стиль ссылок, HTML-сущности и блоки кода с отступом не поддерживаются.
- Глубина вложенности ограничена `max_nest = 4` для цитат/ограждений и для
  жирного/курсива; 5-й уровень той же конструкции остаётся неотслеженным,
  а не представляется собственным спаном.

Подробнее —
[`ARCHITECTURE_RU.md §17`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE_RU.md#17-известные-ограничения-и-намеренные-компромиссы) - *GitHub*
— про оставшиеся компромиссы движка, включая ограничение `max_nest` и
ограничение одного активного `chained`-правила.

---

## Лицензия

`meon-md` доступен под
[***MIT***](https://github.com/vgnapuga/meon/blob/main/LICENSE-MIT) *ИЛИ* [***APACHE-2.0***](https://github.com/vgnapuga/meon/blob/main/LICENSE-APACHE) лицензией - *GitHub*.
