# meon-md — Cross-parser сравнение

[**EN**](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE.md) | RU

Пропускная способность [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
(на движке [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md))
рядом с двумя CommonMark-парсерами, на тех же корпусах, что и intra-engine
бенчи.

> **Эти числа демонстрируют разницу архитектур, а не рейтинг качества.**
> `meon-md` по дизайну **не** соответствует CommonMark — он парсит подмножество
> Markdown в плоскую типизированную таблицу спанов (O(1) доступ на вид
> элементов, извлечение одного типа через `find_*`, zero-copy спаны).
> `pulldown-cmark` и `comrak` — полный CommonMark, они производят поток событий
> / AST. Разрыв в пропускной способности отражает две разные архитектуры с
> разными целями. `Throughput::Bytes` измеряет, как быстро поглощается вход,
> поскольку три решения производят разное.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
  * [***crates.io***](https://crates.io/crates/meon-md)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)
  * ***MD_COMPARE.md***    <--
  * [***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md)
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Что измеряется

Один бинарь, `meon-md_compare`. На корпус (`plain` / `hot` / `heavy`) — три
парсера на идентичном входе, каждый обёрнут в `black_box`:

| Линия            | Вызов                                            | Что производит                                                  |
|------------------|--------------------------------------------------|-----------------------------------------------------------------|
| `meon-md`        | `MarkdownParser::parse`                          | Плоская типизированная таблица спанов для подмножества Markdown.|
| `pulldown-cmark` | `Parser::new(s)`, итератор полностью осушается   | Полный CommonMark event stream, parse-only, без рендера.        |
| `comrak`         | `parse_document(&arena, s, &Options::default())` | Полный CommonMark AST, без рендера. Верхняя граница.            |

`pulldown-cmark` ближе всего по форме к однопроходному скану meon (прямой поток
событий, без владеемого дерева). `comrak` — верхняя граница: он строит
владеемый AST.

Перед замером печатается тот же отчёт о составе корпуса, что и в intra-engine
бенчах.

---

## Почему эти числа демонстрационные, а не рейтинг

- **Несоответствие CommonMark — сознательное.** `meon-md` нацелен на
  подмножество Markdown намеренно; он не является и не стремится быть
  CommonMark-парсером. Его выход — плоская типизированная таблица спанов: O(1)
  доступ на вид элементов, извлечение одного типа через `find_*`, zero-copy
  спаны. Поверх этих спанов можно построить дерево, если потребителю оно нужно.
  Компараторы делают полную работу CommonMark и отдают поток событий / AST.
  Числа сравнивают эти два дизайна.

- **Разница в фичах.** Компараторы обрабатывают ссылочные ссылки, сырой HTML,
  HTML-сущности, блоки кода с отступом, setext-заголовки, приоритет
  ссылок/эмфазы, tight/loose списки и прочее — ничего из этого `meon-md` не
  делает, по дизайну. Они платят за эту поверхность на каждом парсе; meon — нет.

- **Bias корпусов.** Корпусы `plain` / `hot` / `heavy` написаны под набор фич
  `meon-md`, поэтому они недо-нагружают CommonMark-фичи, которые компараторы
  всё равно обрабатывают. Реальные CommonMark-документы сдвигают стоимость
  компараторов относительно показанного здесь.

- **Синтетика как верхняя оценка.** Корпусы программные и однородные.
  Воспринимайте каждую цифру как верхнюю оценку, а не ожидаемую производственную
  пропускную способность.

- **Паритет флагов / SIMD.** meon использует AVX2 только под `--features avx2` +
  `RUSTFLAGS="-C target-cpu=native"`; на stable — скалярный SWAR-путь. У
  `pulldown-cmark` свой opt-in `simd`-сканер (здесь по умолчанию не включён, см.
  [Запуск](#запуск)); `comrak` скалярный. Каждый блок результатов ниже указывает
  точную сборку, под которой снят; рядом стоят только строки с сопоставимыми
  флагами.

- **Формы выхода разные.** SoA-спаны против потока событий против AST.
  `Throughput::Bytes` нормирует по размеру входа — он отвечает на «как быстро
  поглощается вход», поскольку три решения производят разное.

- **Сквозная стоимость.** Тайминг включает собственные аллокации каждого парсера
  (выходные `Vec` у meon, arena у comrak). comrak получает свежую arena на каждой
  итерации; итератор событий pulldown полностью осушается, чтобы ничего не
  пропускалось лениво. Генерация корпуса и `&str`-вид — вне тайминга.

---

## Запуск

Внутри `nix develop`:

```sh
# Stable, скалярный (meon SWAR-путь, pulldown scalar, comrak scalar):
cargo bench --bench meon-md_compare

# Nightly, meon AVX2-путь, оптимизированный под хост:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2
```

Флаги сборки зависимостей (в `benches/Cargo.toml`), выбраны чтобы держать
компараторы на их parse-only пути:

- `pulldown-cmark` - `default-features = false` (убирает `html`-рендер). Чтобы
  дать pulldown его SIMD-сканер для более честной AVX-строки, добавьте
  `features = ["simd"]` и отметьте это в блоке результатов.
- `comrak` - `default-features = false` (убирает `syntect`/рендер-зависимости;
  оставляет `parse_document`, `Arena`, `Options`).

Железо и параметры Criterion общие с intra-engine бенчами — см. *Тестовое
окружение* в
[***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)
и параметры в `benches/benches/docs_md.rs`.

---

## Корпусы

Каждый базовый документ тайлится `REPEAT_COUNT` раз, чтобы рабочий набор
превышал кэш. Прогоны `small` и `big` отличаются только `REPEAT_COUNT`.

| Корпус  | Форма                                                                             | Нагружает                                                                       |
|---------|-----------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `plain` | Только проза, без разметки.                                                       | Путь fallback/текст, цикл строк. Потолочный случай (почти чистое сканирование). |
| `hot`   | Лёгкая, равномерно распределённая разметка (~один элемент каждого вида на абзац). | Типичный реальный документ.                                                     |
| `heavy` | Плотная: заголовки, разделители, цитаты, ограждения, списки, вложенный инлайн.    | Все семейства правил одновременно, включая вложенность. Стресс-случай.          |

> **Синтетические данные.** Все три корпуса сгенерированы программно с
> однородной предсказуемой структурой. В реальных документах **плотность
> элементов обычно ниже** и паттерны менее регулярны, чем в `hot` или `heavy`.
> Воспринимайте числа как верхнюю оценку для вашей конкретной нагрузки, а не
> как ожидаемую производственную пропускную способность.

### Состав корпусов

**small (REPEAT_COUNT = 10)**

```
┌─ corpus: plain
│  size:          2.80 MiB  (2937800 bytes)
│  elements:         2     (0.0 per KiB)
│  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
│
│          headings:         0    thematic_breaks:         0         paragraphs:         1
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:         0            italics:         0
│      bold_italics:         0              codes:         0              links:         0
│         autolinks:         0        hard_breaks:         0              texts:         1
└─

┌─ corpus: hot
│  size:          0.75 MiB  (790600 bytes)
│  elements:     65000     (84.2 per KiB)
│  span mem:      0.50 MiB  (~65.8% of input, 8 B/span lower bound)
│
│          headings:      5000    thematic_breaks:         0         paragraphs:      5000
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:      5000            italics:      5000
│      bold_italics:         0              codes:      5000              links:      5000
│         autolinks:      5000        hard_breaks:         0              texts:     30000
└─

┌─ corpus: heavy
│  size:          1.47 MiB  (1541020 bytes)
│  elements:    140000     (93.0 per KiB)
│  span mem:      1.07 MiB  (~72.7% of input, 8 B/span lower bound)
│
│          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
│       blockquotes:      4000       fenced_codes:      2000       bullet_items:      6000
│     ordered_items:      4000              bolds:     12000            italics:     12000
│      bold_italics:      6000              codes:     10000              links:      6000
│         autolinks:      4000        hard_breaks:         0              texts:     66000
└─
```

**big (REPEAT_COUNT = 1000, превышает L3-кэш)**

```
┌─ corpus: plain
│  size:        280.17 MiB  (293780000 bytes)
│  elements:         2     (0.0 per KiB)
│  span mem:      0.00 MiB  (~0.0% of input, 8 B/span lower bound)
│
│          headings:         0    thematic_breaks:         0         paragraphs:         1
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:         0            italics:         0
│      bold_italics:         0              codes:         0              links:         0
│         autolinks:         0        hard_breaks:         0              texts:         1
└─

┌─ corpus: hot
│  size:         75.40 MiB  (79060000 bytes)
│  elements:   6500000     (84.2 per KiB)
│  span mem:     49.59 MiB  (~65.8% of input, 8 B/span lower bound)
│
│          headings:    500000    thematic_breaks:         0         paragraphs:    500000
│       blockquotes:         0       fenced_codes:         0       bullet_items:         0
│     ordered_items:         0              bolds:    500000            italics:    500000
│      bold_italics:         0              codes:    500000              links:    500000
│         autolinks:    500000        hard_breaks:         0              texts:   3000000
└─

┌─ corpus: heavy
│  size:        146.96 MiB  (154102000 bytes)
│  elements:  14000000     (93.0 per KiB)
│  span mem:    106.81 MiB  (~72.7% of input, 8 B/span lower bound)
│
│          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
│       blockquotes:    400000       fenced_codes:    200000       bullet_items:    600000
│     ordered_items:    400000              bolds:   1200000            italics:   1200000
│      bold_italics:    600000              codes:   1000000              links:    600000
│         autolinks:    400000        hard_breaks:         0              texts:   6600000
└─
```

---

## Результаты

> Пропускная способность (`thrpt`) — главное. Сравнивайте ячейку только с тем же
> корпусом в том же блоке сборки. Каждая ячейка — тройка Criterion `time` /
> `thrpt` (нижняя / медиана / верхняя).

### stable - `cargo bench --bench meon-md_compare`

**small (влезает в кэш):**

| Корпус  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [1.0709 ms 1.0725 ms 1.0744 ms] thrpt: [2.5466 GiB/s 2.5512 GiB/s 2.5549 GiB/s] | time: [3.2574 ms 3.2583 ms 3.2594 ms] thrpt: [859.57 MiB/s 859.85 MiB/s 860.12 MiB/s] | time: [14.646 ms 14.685 ms 14.728 ms] thrpt: [190.23 MiB/s 190.78 MiB/s 191.30 MiB/s] |
| `hot`   | time: [680.44 µs 681.17 µs 681.94 µs] thrpt: [1.0797 GiB/s 1.0809 GiB/s 1.0821 GiB/s] | time: [4.8188 ms 4.8231 ms 4.8274 ms] thrpt: [156.19 MiB/s 156.33 MiB/s 156.47 MiB/s] | time: [18.018 ms 18.092 ms 18.171 ms] thrpt: [41.494 MiB/s 41.675 MiB/s 41.846 MiB/s] |
| `heavy` | time: [1.5665 ms 1.5673 ms 1.5682 ms] thrpt: [937.15 MiB/s 937.71 MiB/s 938.17 MiB/s] | time: [13.503 ms 13.538 ms 13.576 ms] thrpt: [108.25 MiB/s 108.55 MiB/s 108.84 MiB/s] | time: [44.485 ms 44.628 ms 44.777 ms] thrpt: [32.821 MiB/s 32.931 MiB/s 33.037 MiB/s] |

**big (превышает L3-кэш):**

| Корпус  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [101.30 ms 101.41 ms 101.55 ms] thrpt: [2.6943 GiB/s 2.6979 GiB/s 2.7009 GiB/s] | time: [490.65 ms 492.97 ms 495.95 ms] thrpt: [564.92 MiB/s 568.33 MiB/s 571.02 MiB/s] | time: [2.7079 s 2.7419 s 2.7762 s] thrpt: [100.92 MiB/s 102.18 MiB/s 103.46 MiB/s] |
| `hot`   | time: [67.333 ms 68.266 ms 68.775 ms] thrpt: [1.0706 GiB/s 1.0786 GiB/s 1.0935 GiB/s] | time: [849.37 ms 855.51 ms 861.70 ms] thrpt: [87.499 MiB/s 88.132 MiB/s 88.769 MiB/s] | time: [3.6113 s 3.6626 s 3.7174 s] thrpt: [20.282 MiB/s 20.586 MiB/s 20.878 MiB/s] |
| `heavy` | time: [147.91 ms 149.60 ms 151.65 ms] thrpt: [969.10 MiB/s 982.41 MiB/s 993.59 MiB/s] | time: [2.0664 s 2.0760 s 2.0852 s] thrpt: [70.479 MiB/s 70.793 MiB/s 71.120 MiB/s] | time: [7.8153 s 7.8648 s 7.9273 s] thrpt: [18.539 MiB/s 18.686 MiB/s 18.805 MiB/s] |

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_compare --features avx2`

> meon на AVX2; `pulldown-cmark` и `comrak` скалярные (без `simd`-фичи). Колонка
> meon — AVX2 против скалярных компараторов, это не like-for-like SIMD-строка.

**small (влезает в кэш):**

| Корпус  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [629.95 µs 630.73 µs 631.59 µs] thrpt: [4.3320 GiB/s 4.3379 GiB/s 4.3432 GiB/s] | time: [3.8414 ms 3.8427 ms 3.8441 ms] thrpt: [728.84 MiB/s 729.09 MiB/s 729.35 MiB/s] | time: [16.129 ms 16.230 ms 16.329 ms] thrpt: [171.58 MiB/s 172.62 MiB/s 173.71 MiB/s] |
| `hot`   | time: [613.33 µs 614.12 µs 614.94 µs] thrpt: [1.1974 GiB/s 1.1990 GiB/s 1.2005 GiB/s] | time: [4.9585 ms 4.9634 ms 4.9685 ms] thrpt: [151.75 MiB/s 151.91 MiB/s 152.06 MiB/s] | time: [19.804 ms 19.967 ms 20.132 ms] thrpt: [37.452 MiB/s 37.761 MiB/s 38.073 MiB/s] |
| `heavy` | time: [1.3876 ms 1.3894 ms 1.3911 ms] thrpt: [1.0317 GiB/s 1.0330 GiB/s 1.0343 GiB/s] | time: [13.925 ms 13.977 ms 14.029 ms] thrpt: [104.76 MiB/s 105.15 MiB/s 105.54 MiB/s] | time: [49.054 ms 49.363 ms 49.672 ms] thrpt: [29.587 MiB/s 29.772 MiB/s 29.960 MiB/s] |

**big (превышает L3-кэш):**

| Корпус  | `meon-md` | `pulldown-cmark` | `comrak` |
|---------|-----------|------------------|----------|
| `plain` | time: [66.438 ms 66.569 ms 66.753 ms] thrpt: [4.0988 GiB/s 4.1101 GiB/s 4.1182 GiB/s] | time: [588.74 ms 591.30 ms 594.49 ms] thrpt: [471.28 MiB/s 473.82 MiB/s 475.88 MiB/s] | time: [2.8263 s 2.8505 s 2.8776 s] thrpt: [97.363 MiB/s 98.287 MiB/s 99.131 MiB/s] |
| `hot`   | time: [62.814 ms 62.935 ms 63.150 ms] thrpt: [1.1660 GiB/s 1.1699 GiB/s 1.1722 GiB/s] | time: [895.95 ms 901.29 ms 907.51 ms] thrpt: [83.082 MiB/s 83.655 MiB/s 84.154 MiB/s] | time: [3.5513 s 3.6021 s 3.6586 s] thrpt: [20.608 MiB/s 20.931 MiB/s 21.231 MiB/s] |
| `heavy` | time: [135.34 ms 135.72 ms 136.11 ms] thrpt: [1.0544 GiB/s 1.0575 GiB/s 1.0604 GiB/s] | time: [2.1312 s 2.1528 s 2.1730 s] thrpt: [67.631 MiB/s 68.268 MiB/s 68.957 MiB/s] | time: [8.0073 s 8.0551 s 8.1210 s] thrpt: [18.097 MiB/s 18.245 MiB/s 18.354 MiB/s] |

---

## Масштабирование от small к big

Чётче всего разница архитектур видна в том, как каждый парсер держится при росте
входа за пределы кэша (stable-сборка, медианный `thrpt`):

| Парсер           | `plain`              | `hot`                | `heavy`              |
|------------------|----------------------|----------------------|----------------------|
| `meon-md`        | 2.55 -> 2.70 GiB/s   | 1.081 -> 1.079 GiB/s | 938 -> 982 MiB/s     |
| `pulldown-cmark` | 860 -> 568 MiB/s     | 156 -> 88 MiB/s      | 109 -> 71 MiB/s      |
| `comrak`         | 191 -> 102 MiB/s     | 41.7 -> 20.6 MiB/s   | 32.9 -> 18.7 MiB/s   |

- **`meon-md` держит пропускную способность практически плоско** от small к big
  (`plain` и `heavy` даже подрастают). Выход — компактная непрерывная таблица
  спанов (`u32`-пары), поэтому рабочий набор остаётся кэш-дружелюбным при росте
  документа.
- **`pulldown-cmark` теряет ~34–44%** на big — bookkeeping потока событий плюс
  растущий рабочий набор выходят за кэш.
- **`comrak` теряет ~43–51%** и абсолютно медленнее всех на всём диапазоне — он
  материализует владеемый AST, поэтому аллокации и pointer-chasing доминируют
  при росте документа.

Плоская таблица спанов деградирует с масштабом куда меньше, чем поток событий
или владеемое дерево. На AVX2-прогоне картина та же.

---

## meon-md standalone-извлечение (нет аналога у компараторов)

`find_*` сканирует сырой источник ради **одного** вида элементов — например,
всех жирных спанов — без межэлементного контекста. У `pulldown-cmark` и `comrak`
аналога нет: вытащить из них только жирные спаны — значит пройти весь поток
событий или AST. Числа ниже — только meon; они здесь потому, что извлечение по
типу — часть разницы архитектур, о которой этот документ.

Каждая строка отчитывает счётчики `full` vs `standalone`. По дизайну они могут
отличаться: у standalone-скана нет контекста ограждений/экранирования (см.
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).
Замерено на корпусах `small`.

### stable - `cargo bench --bench meon-md_standalone`

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [439.60 µs 439.86 µs 440.19 µs]
                        thrpt:  [6.2156 GiB/s 6.2202 GiB/s 6.2240 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [448.63 µs 449.16 µs 449.60 µs]
                        thrpt:  [6.0855 GiB/s 6.0914 GiB/s 6.0986 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [437.01 µs 439.54 µs 443.74 µs]
                        thrpt:  [6.1659 GiB/s 6.2247 GiB/s 6.2608 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [436.30 µs 437.31 µs 438.64 µs]
                        thrpt:  [6.2375 GiB/s 6.2565 GiB/s 6.2710 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [446.15 µs 447.11 µs 448.62 µs]
                        thrpt:  [6.0988 GiB/s 6.1194 GiB/s 6.1325 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [443.19 µs 443.55 µs 443.92 µs]
                        thrpt:  [6.1634 GiB/s 6.1685 GiB/s 6.1735 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [263.94 µs 265.05 µs 266.29 µs]
                        thrpt:  [10.275 GiB/s 10.323 GiB/s 10.366 GiB/s]

  find_thematic_breaks full=     0  standalone=       0
                        time:   [282.79 µs 283.30 µs 283.76 µs]
                        thrpt:  [9.6420 GiB/s 9.6576 GiB/s 9.6752 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [280.17 µs 280.81 µs 281.93 µs]
                        thrpt:  [9.7046 GiB/s 9.7435 GiB/s 9.7657 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [266.62 µs 266.74 µs 266.85 µs]
                        thrpt:  [10.253 GiB/s 10.257 GiB/s 10.262 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [280.83 µs 280.98 µs 281.15 µs]
                        thrpt:  [9.7316 GiB/s 9.7374 GiB/s 9.7425 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [293.65 µs 294.00 µs 294.41 µs]
                        thrpt:  [9.2933 GiB/s 9.3063 GiB/s 9.3172 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=    5000  standalone=    5000
                        time:   [245.28 µs 245.50 µs 245.74 µs]
                        thrpt:  [2.9962 GiB/s 2.9992 GiB/s 3.0019 GiB/s]

  find_italics       full=    5000  standalone=    5000
                        time:   [287.68 µs 287.82 µs 287.95 µs]
                        thrpt:  [2.5571 GiB/s 2.5582 GiB/s 2.5594 GiB/s]

  find_bolds         full=    5000  standalone=    5000
                        time:   [299.68 µs 299.96 µs 300.27 µs]
                        thrpt:  [2.4521 GiB/s 2.4547 GiB/s 2.4570 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [277.21 µs 278.00 µs 279.04 µs]
                        thrpt:  [2.6387 GiB/s 2.6486 GiB/s 2.6561 GiB/s]

  find_autolinks     full=    5000  standalone=    5000
                        time:   [256.79 µs 258.35 µs 259.42 µs]
                        thrpt:  [2.8383 GiB/s 2.8500 GiB/s 2.8673 GiB/s]

  find_links         full=    5000  standalone=    5000
                        time:   [274.13 µs 275.12 µs 276.79 µs]
                        thrpt:  [2.6602 GiB/s 2.6763 GiB/s 2.6859 GiB/s]

  find_headings      full=    5000  standalone=    5000
                        time:   [119.65 µs 119.74 µs 119.83 µs]
                        thrpt:  [6.1447 GiB/s 6.1492 GiB/s 6.1540 GiB/s]

  find_thematic_breaks full=     0  standalone=       0
                        time:   [126.00 µs 126.07 µs 126.16 µs]
                        thrpt:  [5.8361 GiB/s 5.8405 GiB/s 5.8439 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [125.04 µs 125.12 µs 125.19 µs]
                        thrpt:  [5.8814 GiB/s 5.8847 GiB/s 5.8883 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [118.54 µs 119.20 µs 119.85 µs]
                        thrpt:  [6.1434 GiB/s 6.1771 GiB/s 6.2116 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [126.14 µs 126.79 µs 127.46 µs]
                        thrpt:  [5.7768 GiB/s 5.8074 GiB/s 5.8370 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [130.59 µs 130.78 µs 131.15 µs]
                        thrpt:  [5.6143 GiB/s 5.6300 GiB/s 5.6382 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full=   10000  standalone=   10000
                        time:   [565.36 µs 565.77 µs 566.35 µs]
                        thrpt:  [2.5341 GiB/s 2.5367 GiB/s 2.5386 GiB/s]

  find_italics       full=   12000  standalone=   12000
                        time:   [794.70 µs 803.03 µs 814.77 µs]
                        thrpt:  [1.7615 GiB/s 1.7872 GiB/s 1.8059 GiB/s]

  find_bolds         full=   12000  standalone=   12000
                        time:   [797.14 µs 797.89 µs 798.53 µs]
                        thrpt:  [1.7973 GiB/s 1.7987 GiB/s 1.8004 GiB/s]

  find_bold_italics  full=    6000  standalone=    6000
                        time:   [819.52 µs 821.18 µs 822.57 µs]
                        thrpt:  [1.7448 GiB/s 1.7477 GiB/s 1.7512 GiB/s]

  find_autolinks     full=    4000  standalone=    4000
                        time:   [497.27 µs 499.20 µs 501.02 µs]
                        thrpt:  [2.8645 GiB/s 2.8750 GiB/s 2.8862 GiB/s]

  find_links         full=    6000  standalone=    6000
                        time:   [550.56 µs 551.07 µs 551.67 µs]
                        thrpt:  [2.6015 GiB/s 2.6044 GiB/s 2.6068 GiB/s]

  find_headings      full=    2000  standalone=    2000
                        time:   [247.47 µs 247.66 µs 247.88 µs]
                        thrpt:  [5.7898 GiB/s 5.7950 GiB/s 5.7995 GiB/s]

  find_thematic_breaks full=  2000  standalone=    2000
                        time:   [265.19 µs 266.33 µs 267.99 µs]
                        thrpt:  [5.3553 GiB/s 5.3887 GiB/s 5.4118 GiB/s]

  find_fenced_codes  full=    2000  standalone=    2000
                        time:   [277.96 µs 278.37 µs 278.95 µs]
                        thrpt:  [5.1450 GiB/s 5.1557 GiB/s 5.1633 GiB/s]

  find_blockquotes   full=    4000  standalone=    2000
                        time:   [255.04 µs 255.15 µs 255.29 µs]
                        thrpt:  [5.6218 GiB/s 5.6250 GiB/s 5.6272 GiB/s]

  find_bullet_items  full=    6000  standalone=    6000
                        time:   [271.36 µs 271.66 µs 271.96 µs]
                        thrpt:  [5.2772 GiB/s 5.2830 GiB/s 5.2889 GiB/s]

find_ordered_items full=    4000  standalone=    4000
                        time:   [287.23 µs 287.47 µs 287.69 µs]
                        thrpt:  [4.9886 GiB/s 4.9925 GiB/s 4.9966 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

<details>
<summary>plain</summary>

```
  find_codes         full=       0  standalone=       0
                        time:   [323.61 µs 323.95 µs 324.37 µs]
                        thrpt:  [8.4349 GiB/s 8.4460 GiB/s 8.4547 GiB/s]

  find_italics       full=       0  standalone=       0
                        time:   [323.64 µs 324.19 µs 324.97 µs]
                        thrpt:  [8.4193 GiB/s 8.4397 GiB/s 8.4538 GiB/s]

  find_bolds         full=       0  standalone=       0
                        time:   [325.06 µs 325.96 µs 327.01 µs]
                        thrpt:  [8.3668 GiB/s 8.3937 GiB/s 8.4171 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [323.56 µs 324.00 µs 324.72 µs]
                        thrpt:  [8.4259 GiB/s 8.4446 GiB/s 8.4559 GiB/s]

  find_autolinks     full=       0  standalone=       0
                        time:   [323.77 µs 327.77 µs 333.69 µs]
                        thrpt:  [8.1994 GiB/s 8.3476 GiB/s 8.4506 GiB/s]

  find_links         full=       0  standalone=       0
                        time:   [331.43 µs 332.72 µs 333.51 µs]
                        thrpt:  [8.2037 GiB/s 8.2233 GiB/s 8.2553 GiB/s]

  find_headings      full=       0  standalone=       0
                        time:   [242.73 µs 242.96 µs 243.18 µs]
                        thrpt:  [11.251 GiB/s 11.261 GiB/s 11.272 GiB/s]

  find_thematic_breaks full=     0  standalone=       0
                        time:   [262.27 µs 262.57 µs 262.85 µs]
                        thrpt:  [10.409 GiB/s 10.420 GiB/s 10.432 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [266.07 µs 266.18 µs 266.30 µs]
                        thrpt:  [10.274 GiB/s 10.279 GiB/s 10.283 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [247.03 µs 247.18 µs 247.33 µs]
                        thrpt:  [11.062 GiB/s 11.069 GiB/s 11.076 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [262.08 µs 262.29 µs 262.49 µs]
                        thrpt:  [10.423 GiB/s 10.431 GiB/s 10.440 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [280.73 µs 280.93 µs 281.15 µs]
                        thrpt:  [9.7317 GiB/s 9.7391 GiB/s 9.7463 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
  find_codes         full=    5000  standalone=    5000
                        time:   [181.20 µs 181.29 µs 181.38 µs]
                        thrpt:  [4.0594 GiB/s 4.0614 GiB/s 4.0634 GiB/s]

  find_italics       full=    5000  standalone=    5000
                        time:   [233.56 µs 233.64 µs 233.73 µs]
                        thrpt:  [3.1502 GiB/s 3.1514 GiB/s 3.1526 GiB/s]

  find_bolds         full=    5000  standalone=    5000
                        time:   [229.64 µs 229.80 µs 229.98 µs]
                        thrpt:  [3.2016 GiB/s 3.2041 GiB/s 3.2063 GiB/s]

  find_bold_italics  full=       0  standalone=       0
                        time:   [232.17 µs 233.02 µs 234.30 µs]
                        thrpt:  [3.1426 GiB/s 3.1598 GiB/s 3.1714 GiB/s]

  find_autolinks     full=    5000  standalone=    5000
                        time:   [205.88 µs 206.55 µs 207.70 µs]
                        thrpt:  [3.5450 GiB/s 3.5647 GiB/s 3.5764 GiB/s]

  find_links         full=    5000  standalone=    5000
                        time:   [204.68 µs 204.82 µs 205.01 µs]
                        thrpt:  [3.5916 GiB/s 3.5949 GiB/s 3.5973 GiB/s]

  find_headings      full=    5000  standalone=    5000
                        time:   [110.70 µs 110.92 µs 111.16 µs]
                        thrpt:  [6.6235 GiB/s 6.6383 GiB/s 6.6513 GiB/s]

  find_thematic_breaks full=     0  standalone=       0
                        time:   [118.78 µs 118.86 µs 118.94 µs]
                        thrpt:  [6.1906 GiB/s 6.1945 GiB/s 6.1988 GiB/s]

  find_fenced_codes  full=       0  standalone=       0
                        time:   [119.62 µs 119.68 µs 119.75 µs]
                        thrpt:  [6.1486 GiB/s 6.1520 GiB/s 6.1556 GiB/s]

  find_blockquotes   full=       0  standalone=       0
                        time:   [110.93 µs 110.97 µs 111.01 µs]
                        thrpt:  [6.6326 GiB/s 6.6354 GiB/s 6.6378 GiB/s]

  find_bullet_items  full=       0  standalone=       0
                        time:   [119.21 µs 119.28 µs 119.34 µs]
                        thrpt:  [6.1696 GiB/s 6.1731 GiB/s 6.1763 GiB/s]

  find_ordered_items full=       0  standalone=       0
                        time:   [122.73 µs 122.79 µs 122.83 µs]
                        thrpt:  [5.9944 GiB/s 5.9965 GiB/s 5.9992 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
  find_codes         full=   10000  standalone=   10000
                        time:   [420.02 µs 420.35 µs 420.74 µs]
                        thrpt:  [3.4111 GiB/s 3.4142 GiB/s 3.4169 GiB/s]

  find_italics       full=   12000  standalone=   12000
                        time:   [613.72 µs 614.16 µs 614.59 µs]
                        thrpt:  [2.3352 GiB/s 2.3368 GiB/s 2.3385 GiB/s]

  find_bolds         full=   12000  standalone=   12000
                        time:   [613.02 µs 613.97 µs 615.20 µs]
                        thrpt:  [2.3329 GiB/s 2.3375 GiB/s 2.3412 GiB/s]

  find_bold_italics  full=    6000  standalone=    6000
                        time:   [610.56 µs 612.49 µs 614.09 µs]
                        thrpt:  [2.3371 GiB/s 2.3432 GiB/s 2.3506 GiB/s]

  find_autolinks     full=    4000  standalone=    4000
                        time:   [368.56 µs 369.27 µs 369.81 µs]
                        thrpt:  [3.8808 GiB/s 3.8865 GiB/s 3.8940 GiB/s]

  find_links         full=    6000  standalone=    6000
                        time:   [405.21 µs 405.44 µs 405.65 µs]
                        thrpt:  [3.5380 GiB/s 3.5398 GiB/s 3.5419 GiB/s]

  find_headings      full=    2000  standalone=    2000
                        time:   [232.59 µs 232.73 µs 232.94 µs]
                        thrpt:  [6.1613 GiB/s 6.1667 GiB/s 6.1704 GiB/s]

  find_thematic_breaks full=  2000  standalone=    2000
                        time:   [245.32 µs 245.56 µs 245.92 µs]
                        thrpt:  [5.8360 GiB/s 5.8446 GiB/s 5.8504 GiB/s]

  find_fenced_codes  full=    2000  standalone=    2000
                        time:   [254.46 µs 254.66 µs 254.82 µs]
                        thrpt:  [5.6321 GiB/s 5.6357 GiB/s 5.6402 GiB/s]

  find_blockquotes   full=    4000  standalone=    2000
                        time:   [234.97 µs 235.08 µs 235.18 µs]
                        thrpt:  [6.1026 GiB/s 6.1050 GiB/s 6.1080 GiB/s]

  find_bullet_items  full=    6000  standalone=    6000
                        time:   [247.28 µs 247.45 µs 247.60 µs]
                        thrpt:  [5.7963 GiB/s 5.8000 GiB/s 5.8038 GiB/s]

  find_ordered_items full=    4000  standalone=    4000
                        time:   [263.64 µs 263.79 µs 263.94 µs]
                        thrpt:  [5.4375 GiB/s 5.4405 GiB/s 5.4438 GiB/s]
```

</details>

---

## Как читать числа

- Числа показывают разницу архитектур (плоские типизированные спаны против
  потока событий против AST) и сознательную нацеленность `meon-md` на
  подмножество Markdown. Потребитель, которому нужно дерево, может построить его
  поверх спанов meon.
- Сравнивайте ячейку только с тем же корпусом в том же блоке сборки.
- `pulldown-cmark` — ближайшая по форме пара; `comrak` — верхняя граница (он
  владеет деревом). Зазор между ними ограничивает стоимость построения AST
  поверх чистого потока событий.
- **Масштабирование — главный сигнал.** meon держит ПС плоско от small к big;
  компараторы теряют 34–51%. Кэш-резидентной остаётся именно плоская таблица
  спанов.
- Корпусы написаны под подмножество `meon-md`; реальная CommonMark-нагрузка
  сдвигает стоимость компараторов относительно показанного.
