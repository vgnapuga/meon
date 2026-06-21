# meon — Фузз-тестирование

[**EN**](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md) | RU

Фузз-тестирование с покрытием для движка [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
через референсную грамматику [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md),
с использованием [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (libFuzzer).

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
* ***FUZZING.md***    <--

---

## Цель

Единственная цель — `parse_text` (`fuzz/fuzz_targets/parse_text.rs`): передаёт
произвольные байты в `MarkdownParser::parse` и проверяет основной **инвариант
корректности спанов** для каждого вида элементов в результирующей контент-структуре.

```rust
fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_LEN {
        return;
    }
    let c = MarkdownParser::parse(data);
    let check = |start: u32, end: u32| {
        assert!(start <= end);
        let _ = &c.source[start as usize..end as usize];  // паника при OOB
    };
    // все виды элементов: texts, bolds, italics, bold_italics, codes,
    // autolinks, hard_breaks, links, paragraphs, blockquotes, fenced_codes,
    // headings, thematic_breaks, bullet_items, ordered_items
});
```

Упражняется полное подмножество Markdown из `meon-md`, так что фаззер
задействует все семейства правил движка (inline, line, block) за один проход.

---

## Проверяемые инварианты

Для каждого спана, произведённого парсером:

- `start <= end` — спаны являются корректными полуоткрытыми диапазонами.
- `source[start..end]` не паникует — спаны никогда не выходят за пределы входа.
- Якоря жёстких переносов строк нулевой длины: `start == end`.
- Входы длиннее `MAX_INPUT_LEN` (`u32::MAX`, 4 ГБ) пропускаются сразу,
  поскольку `u32`-смещения не могут их представить (см.
  [`ARCHITECTURE.md §14`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#14-span-representation)).

Замыкание `check` использует реальную индексацию среза (`&c.source[start..end]`),
а не числовое сравнение — поэтому любой выход за пределы вызывает немедленную
панику, которую libFuzzer перехватывает и сохраняет как воспроизводящий артефакт.

---

## Требования

- **Nightly**-тулчейн (libFuzzer требует санитайзер-рантайм).
- `cargo-fuzz`.

Оба доступны в dev-шелле:

```sh
nix develop
```

За пределами Nix:

```sh
rustup toolchain install nightly
cargo install cargo-fuzz
```

---

## Запуск

```sh
# Список целей:
cargo fuzz list

# Запуск до прерывания (Ctrl-C):
cargo fuzz run parse_text

# С ограничением по времени:
cargo fuzz run parse_text -- -max_total_time=7200

# Без AddressSanitizer (в 2-4 раза быстрее; безопасно — крейт не содержит unsafe):
cargo fuzz run parse_text --sanitizer none -- -max_total_time=7200

# С сидами из бенчмарковых корпусов (рекомендуется — ускоряет рост покрытия):
cargo fuzz run parse_text fuzz/corpus/parse_text
```

---

## Триаж

```sh
# Повторить сохранённый краш:
cargo fuzz run parse_text fuzz/artifacts/parse_text/<crash-file>

# Минимизировать краш до наименьшего воспроизводящего входа:
cargo fuzz tmin parse_text fuzz/artifacts/parse_text/<crash-file>

# Прогнать корпус без фаззинга (регрессионная проверка):
cargo fuzz run parse_text fuzz/corpus/parse_text -- -runs=0
```

Папки `corpus/`, `artifacts/`, `coverage/` и `target/` игнорируются git
(`fuzz/.gitignore`).

---

## Лог кампаний

| версия релиза  | дата       | тулчейн            | всего итераций | cov  | ft   | corp       | exec/s | rss   |
|----------------|------------|--------------------|----------------|------|------|------------|--------|-------|
| v0.1.0         | 2026-06-15 | nightly-2026-05-22 | ~104M          | 841  | 4766 | 1758/252Kb | ~35k   | 629Mb |
| v0.2.0         | 2026-06-21 | nightly-2026-05-22 | ~111M          | 1114 | 6853 | 2346/440Kb | ~32k   | 641Mb |

**Насыщение покрытия** на `cov: 1114 ft: 6853 corp: 1758/252Kb` означает что
libFuzzer исчерпал достижимые ветки на случайных входах без сидов. Добавление
сид-документов из реальных Markdown-файлов или из бенчмарковых корпусов поднимет
покрытие выше — фаззер получит точку входа в структурированные пути исполнения.

---

## Примечания

- Крейт **не содержит `unsafe`-кода** (`#![forbid(unsafe_code)]`).
  AddressSanitizer для целей безопасности памяти избыточен и может быть
  отключён через `--sanitizer none` для ускорения в 2–4 раза.
- Standalone-итераторы `find_*` не упражняются этой целью — они сканируют
  сырые байты независимо, их граничные условия покрыты юнит-тестами. При
  расширении `_raw`-акцессоров можно добавить отдельную фазз-цель.
- `avx512` не тестировался в ходе фаззинга — железо с AVX-512 было недоступно.
  Скалярный путь и путь `avx2` покрыты.
