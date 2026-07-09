# meon — Бенчмарки

[**EN**](https://github.com/vgnapuga/meon/blob/main/benches/README.md) | RU

Бенчмарки пропускной способности для грамматик [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
и [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md),
построенных на движке [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).
Цель — отслеживать производительность движка между изменениями и при переключении
feature-флагов, **а не** занимать место в рейтинге среди других парсеров (см.
[Честность измерений](#честность-измерений)).

| Бенч                  | Измеряет                                                                    |
|-----------------------|-----------------------------------------------------------------------------|
| `meon-md_parse`       | `MarkdownParser::parse` — полный однопроходной парс.                        |
| `meon-md_standalone`  | Итераторы `find_*` — один вид элементов, без контекста.                     |
| `meon-md_compare`     | meon-md против `pulldown-cmark` / `comrak` — cross-parser ПС.               |
| `meon-json_parse`     | `JsonParser::parse` (+ `type_scalars`) — структурный / типизированный парс. |
| `meon-json_compare`   | meon-json против `simd-json` / `sonic-rs` — cross-parser ПС.                |

Отчёты о составе корпусов, полные таблицы результатов и cross-parser числа живут
в своих документах — этот файл это обзор, как-запускать, рамка честности и
тестовое окружение. Cross-parser сравнения —
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md)
(Markdown) и
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE_RU.md)
(JSON).

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
* ***BENCHMARKS.md***    <--
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md)
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Корпусы

### Markdown-корпусы (`meon-md_*`)

Каждый базовый документ тайлится `REPEAT_COUNT` раз (по умолчанию `10`), чтобы
рабочий набор заведомо превышал кэш процессора.

| Корпус  | Форма                                                                             | Нагружает                                                                       |
|---------|-----------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `plain` | Только проза, без разметки.                                                       | Путь fallback/текст, цикл строк. Потолочный случай (почти чистое сканирование). |
| `hot`   | Лёгкая, равномерно распределённая разметка (~один элемент каждого вида на абзац). | Типичный реальный документ.                                                     |
| `heavy` | Плотная: заголовки, разделители, цитаты, ограждения, списки, вложенный инлайн.    | Все семейства правил одновременно, включая вложенность. Стресс-случай.          |

> **Синтетические данные.** Эти корпусы сгенерированы программно с однородной
> предсказуемой структурой. В реальных документах **плотность элементов обычно
> ниже**, чем в `hot` или `heavy`, — а меньшая плотность это меньше работы на
> элемент, поэтому реальная пропускная способность обычно оказывается **на
> уровне или выше** чисел `hot`/`heavy`. Читайте `hot`/`heavy` как
> консервативную нижнюю границу, с `plain` (без разметки) как потолком, а вашу
> нагрузку — где-то между.

Точные счётчики элементов по корпусам — в
[***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md).

### JSON-корпусы (`meon-json_*`)

Каждый корпус — один валидный JSON-массив верхнего уровня, масштабируемый через
`COUNT` (`benches/benches/docs_json.rs`); прогоны `small` и `big` отличаются
только `COUNT`.

| Корпус    | Форма                                                                          | Нагружает                                                                 |
|-----------|--------------------------------------------------------------------------------|---------------------------------------------------------------------------|
| `numbers` | Плоский массив чисел / bool / null.                                            | Скан скаляров. meon делает меньше всех; валидирующие парсят каждое число. |
| `objects` | Массив плоских объектов со смешанно-типизированными полями.                    | Члены, ключи, типизированные скаляры. Типичная API-нагрузка.              |
| `nested`  | Массив умеренно вложенных объектов (объекты-в-объектах, мелкие массивы).       | Унифицированный стек вложенности и правило строк.                         |

> **Синтетические данные.** Эти корпусы сгенерированы программно с однородной
> структурой; реальный JSON менее регулярен. Воспринимайте числа как
> демонстрацию разницы архитектур (структурный ридер против валидирующего
> парсера), а не ожидаемую производственную пропускную способность.

Точный состав корпусов — в
[***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE_RU.md).

---

## Запуск

Внутри `nix develop`:

```sh
# Stable, скалярный SWAR-путь:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone
cargo bench --bench meon-md_compare
cargo bench --bench meon-json_parse
cargo bench --bench meon-json_compare

# Nightly, SIMD-путь AVX2, оптимизированный под хост:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse        --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone   --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare    --features avx2
```

Параметры Criterion (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) находятся в
`benches/benches/docs_md.rs` и `benches/benches/docs_json.rs`. Дефолты
рассчитаны на быстрый локальный прогон; для публикационных чисел их стоит
увеличить.

---

## Честность измерений

- **Сначала intra-engine.** `meon-md_parse` / `meon-md_standalone` /
  `meon-json_parse` измеряют *этот* движок на *этих* корпусах. Они имеют смысл
  для «регрессировало ли моё изменение?» и «насколько помогает AVX2?», но не для
  таблицы лидеров.
- **Cross-parser сравнения — архитектурные, не рейтинги.** `meon-md` возвращает
  плоские спаны для *подмножества* Markdown (без AST, резолва ссылок, рендера);
  `meon-json` — *структурный ридер* (без валидации, парсинга чисел и
  разэкранирования строк). Сравнения — против `pulldown-cmark` / `comrak`
  ([***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md))
  и против `simd-json` / `sonic-rs`
  ([***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE_RU.md))
  — поданы там как разница архитектур, а не рейтинг качества.
- **Сквозная стоимость.** Тайминг включает внутренние аллокации `Vec`, потому
  что именно это платит реальный вызывающий код. Вход и выход обёрнуты в
  `black_box`; генерация документа вынесена за пределы замера.

---

## Заметки о производительности

- **У `max_nest` плоская стоимость на строку.** Стек активных блоков на блочном
  уровне и ограниченные стеки инлайн-движка — это массивы `[T; max_nest]`,
  обнуляемые при **каждом** вызове `parse_block!` / `parse_inline!`, независимо
  от того, вкладывается ли реально эта строка. Ставьте `max_nest` в минимальное
  значение, которое нужно вашей грамматике (у `meon-md` это `4`, у `meon-json`
  — `64`); больший предел стоит пропускной способности на каждой строке с
  инлайном, вкладывается там что-то или нет.
- **AVX-512 реализован, но не замерялся.** Фича `avx512` есть (см.
  [`swar.rs`](https://github.com/vgnapuga/meon/blob/main/meon/src/swar.rs)), но
  железо с AVX-512 во время разработки было недоступно. Контрибуции с реальными
  числами приветствуются.

---

## Тестовое окружение

```
CPU:             AMD Ryzen 5 5625U (Zen 3)
RAM:             16 GB
OS:              NixOS 25.05
rustc (stable):  1.86.0
rustc (nightly): 1.98.0-nightly
Environment:     nix develop (isolated shell)
```
