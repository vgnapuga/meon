# meon-macros

[**EN**](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md) | RU

Процедурный макрос-крейт для движка парсинга `meon`.

Этот крейт предоставляет единственную публичную точку входа — `define_parser!` — которая компилирует декларативное описание грамматики в полноценный парсер во время сборки. Использовать его напрямую не нужно: добавьте зависимость от `meon`, который реэкспортирует `define_parser!` и предоставляет всю рантайм-инфраструктуру.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**    <--
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md) - *GitHub*
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README.md) - *GitHub*
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) - *GitHub*

---

## Использование

Не добавляйте `meon-macros` в `Cargo.toml` напрямую.

```toml
[dependencies]
meon = "0.2"
```

```rust
use meon::define_parser;

define_parser!(MyFormat {
    sep = b' ', eol = b'\n', tab = b'\t', escape = b'\\';

    inline {
        fallback => texts [10];
    }
    blocks {
        fallback => paragraphs [80];
    }
});

let content = MyFormatParser::parse(b"hello world\n");
```

Полный справочник по синтаксису грамматики — в
[`meon/README.md`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).

---

## Что генерирует `define_parser!`

Для вызова `define_parser!(Name { ... })` макрос разворачивается в:

**`NameContent<'a>`** — выходная структура. По одному `pub`-полю на каждое правило грамматики, все заимствуют оригинальный срез источника через `u32` байтовые смещения-спаны.

**`NameParser`** — unit-структура с методами:
- `parse(source: &[u8]) -> NameContent<'_>` — полный однопроходной O(n) парс.
- `find_<field>(source: &[u8]) -> impl Iterator` — standalone-итератор на каждое правило грамматики которое это поддерживает. Контекст-свободный и быстрее полного парса когда нужен только один вид элементов.

**Аксессорные методы на `NameContent`:**
- `str(span) -> Option<&str>` — спан в UTF-8 строку, `None` при невалидном UTF-8.
- `bytes(span) -> &[u8]` — спан в байтовый срез.
- `<field>_clean()` — итератор по внутренним срезам содержимого (между разделителями).
- `<field>_raw()` — итератор по полным срезам включая байты разделителей.

---

## Внутренний пайплайн

`define_parser!` — тонкая обёртка над трёхэтапным пайплайном времени компиляции, реализованным целиком в `meon-macros`:

```
Токены DSL грамматики
    │
    ▼
[cursor.rs]   самодельный курсор TokenStream
    │
    ▼
[collect.rs]  фронтенд грамматики
    │          заполняет CF (собранные поля) + Vec<StandaloneRule>
    ▼
[strip.rs]    удаляет аннотации => field [N]
    │          чтобы очищенные токены прошли в рантайм-макросы
    ▼
[codegen.rs]  эмитирует вызов define_content!(...)
[methods.rs]  эмитирует impl с аксессорами _clean / _raw
[codegen.rs]  эмитирует вызов define_standalone_fns! { ... }
    │
    ▼
Финальный поток токенов → rustc
```

Всё рантайм-поведение живёт в `meon` (декларативные макросы `parse_text!`,
`parse_inline!`, `parse_line!`, `parse_block!` и `define_standalone_fns!`).
`meon-macros` только производит токены — у него нет рантайм-следа.

Подробное описание каждого этапа — в
[`ARCHITECTURE.md §4`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#4-grammar-compilation-pipeline) - *GitHub*.

---

## Сообщения об ошибках

Некорректная грамматика вызывает `compile_error!` с привязкой к месту в исходнике вместо паники процедурного макроса. Пример:

```
error: expected literal (fence min)
 --> src/lib.rs:7:35
  |
7 |     blocks { block_simple { fence(b'`') => fenced_codes [400]; } }
  |                                   ^^^^
```

---

## Кросс-крейтовая гигиена

Все вызовы макросов в сгенерированном коде полностью квалифицированы через
`proc_macro_crate::crate_name` — сгенерированный код разрешается корректно
независимо от того как `meon` импортирован или переименован в `Cargo.toml`.

Подробное объяснение — в
[`ARCHITECTURE.md §16`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#16-cross-crate-macro-hygiene) - *GitHub*.

---

## Лицензия

`meon-macros` доступен под лицензией
[***GNU Affero General Public License v3.0 (AGPL-3.0)***](https://github.com/vgnapuga/meon/blob/main/LICENSE) - *GitHub*.

Если условия AGPL-3.0 несовместимы с вашим сценарием использования, доступна коммерческая лицензия — см. [***COMMERCIAL.md***](https://github.com/vgnapuga/meon/blob/main/COMMERCIAL.md) - *GitHub*.

Внося вклад в проект, вы соглашаетесь с [***Соглашением о лицензировании контрибуций (CLA)***](https://github.com/vgnapuga/meon/blob/main/CLA.md) - *GitHub*.
