# meon-md — Бенчмарки

[**EN**](https://github.com/vgnapuga/meon/blob/main/meon/benches/README.md) | RU

Бенчмарки пропускной способности для грамматики [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md),
построенной на движке [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).
Цель — отслеживать производительность движка между изменениями и при переключении
feature-флагов, **а не** занимать место в рейтинге среди других парсеров (см.
[Честность измерений](#честность-измерений)).

Два бенчмарк-бинаря:

| Бенч                 | Измеряет                                                        |
|----------------------|-----------------------------------------------------------------|
| `meon-md_parse`      | `MarkdownParser::parse` — полный однопроходной парс.            |
| `meon-md_standalone` | Итераторы `find_*` — один вид элементов, без контекста.         |

Оба печатают **отчёт о размере и составе корпуса** перед замером, чтобы каждое
число пропускной способности можно было читать в контексте того, сколько и какой
структуры парсер реально произвёл.

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
* ***BENCHMARKS.md***    <--
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README.md)

---

## Корпусы

Каждый базовый документ тайлится `REPEAT_COUNT` раз (по умолчанию `10`), чтобы
рабочий набор заведомо превышал кэш процессора.

| Корпус  | Форма                                                                             | Нагружает                                                                       |
|---------|-----------------------------------------------------------------------------------|---------------------------------------------------------------------------------|
| `plain` | Только проза, без разметки.                                                       | Путь fallback/текст, цикл строк. Потолочный случай (почти чистое сканирование). |
| `hot`   | Лёгкая, равномерно распределённая разметка (~один элемент каждого вида на абзац). | Типичный реальный документ.                                                     |
| `heavy` | Плотная: заголовки, разделители, цитаты, ограждения, списки, вложенный инлайн.    | Все семейства правил одновременно. Стресс-случай.                               |

> **Синтетические данные.** Все три корпуса сгенерированы программно с
> однородной предсказуемой структурой. В реальных документах **плотность
> элементов обычно ниже** и паттерны менее регулярны, чем в `hot` или `heavy`.
> Воспринимайте числа как верхнюю оценку для вашей конкретной нагрузки, а не
> как ожидаемую производственную пропускную способность.

---

## Запуск

Внутри `nix develop`:

```sh
# Stable, скалярный SWAR-путь:
cargo bench --bench meon-md_parse
cargo bench --bench meon-md_standalone

# Nightly, SIMD-путь AVX2, оптимизированный под хост:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse      --features avx2
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2
```

Параметры Criterion (`SAMPLE_SIZE`, `SAMPLE_TIME`, `WARMUP_TIME`) находятся в
`benches/benches/docs_md.rs`. Дефолты рассчитаны на быстрый локальный прогон;
для публикационных чисел их стоит увеличить.

---

## Честность измерений

- **Только внутри движка.** Числа измеряют *этот* движок на *этих* корпусах.
  Они имеют смысл для «регрессировало ли моё изменение?» и «насколько помогает
  AVX2?», но не для таблицы лидеров.
- **Несравнимо с CommonMark-парсерами напрямую.** `meon-md` возвращает плоские
  спаны для *подмножества* Markdown и не строит AST, не резолвит ссылки и не
  рендерит. Честное сравнение с `pulldown-cmark` / `comrak` потребует
  зафиксировать оба на parse-only и задокументировать разницу в фичах.
- **Сквозная стоимость.** Тайминг включает внутренние аллокации `Vec`, потому
  что именно это платит реальный вызывающий код. Вход и выход обёрнуты в
  `black_box`; генерация документа вынесена за пределы замера.

---

## Известные характеристики производительности

**Пропускная способность не линейна при масштабировании.** Парсер
предварительно выделяет ёмкость `Vec` как `source.len() / div`. Когда
накопленные выходные `Vec`-ы вырастают настолько, что перестают помещаться в
кэш последнего уровня процессора, узким местом становится давление на аллокатор
и кэш-промахи, а не скорость сканирования. Это видно в сравнении small → big
ниже: пропускная способность падает на ~30–35% когда рабочий набор не влезает
в кэш.

**Способы сгладить** (без изменений в `meon`):

- Заменить глобальный аллокатор на [`mimalloc`](https://crates.io/crates/mimalloc)
  или [`jemallocator`](https://crates.io/crates/jemallocator) в крейте-потребителе.
  Оба снижают накладные расходы на аллокации при большом масштабе.
- Подобрать делители ёмкости `[div]` в грамматике под реальную плотность
  элементов в ваших данных. Более точная предаллокация — меньшие `Vec`-ы и
  меньшее давление на кэш.

**AVX-512 не тестировался.** Фича `avx512` реализована (см.
[`swar.rs`](https://github.com/vgnapuga/meon/blob/main/meon/src/swar.rs)),
но не замерялась — железо с AVX-512 во время разработки было недоступно.
Вклад с реальными числами приветствуется.

---

## Тестовое окружение

```
CPU:             AMD Ryzen 5 5625U (Zen 3)
RAM:             16 GB
ОС:              NixOS 25.05
rustc (stable):  1.86.0
rustc (nightly): 1.98.0-nightly
Окружение:       nix develop (изолированный шелл)
```

---

## Состав корпусов

### small (REPEAT_COUNT = 10)

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
│  size:          1.17 MiB  (1223220 bytes)
│  elements:    126000     (105.5 per KiB)
│  span mem:      0.96 MiB  (~82.4% of input, 8 B/span lower bound)
│
│          headings:      2000    thematic_breaks:      2000         paragraphs:      4000
│       blockquotes:      2000       fenced_codes:      2000       bullet_items:      6000
│     ordered_items:      4000              bolds:     10000            italics:      8000
│      bold_italics:      6000              codes:     10000              links:      6000
│         autolinks:      4000        hard_breaks:         0              texts:     60000
└─
```

### big (REPEAT_COUNT = 1000, превышает L3-кэш)

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
│  size:        116.66 MiB  (122322000 bytes)
│  elements:  12600000     (105.5 per KiB)
│  span mem:     96.13 MiB  (~82.4% of input, 8 B/span lower bound)
│
│          headings:    200000    thematic_breaks:    200000         paragraphs:    400000
│       blockquotes:    200000       fenced_codes:    200000       bullet_items:    600000
│     ordered_items:    400000              bolds:   1000000            italics:    800000
│      bold_italics:    600000              codes:   1000000              links:    600000
│         autolinks:    400000        hard_breaks:         0              texts:   6000000
└─
```

---

## Результаты — parse (`meon-md_parse`)

### stable — `cargo bench --bench meon-md_parse`

**small (помещается в кэш):**

```
parse/plain/full        time:   [1.0975 ms 1.0978 ms 1.0980 ms]
                        thrpt:  [2.4918 GiB/s 2.4923 GiB/s 2.4929 GiB/s]

parse/hot/full          time:   [638.18 µs 638.44 µs 638.72 µs]
                        thrpt:  [1.1528 GiB/s 1.1533 GiB/s 1.1538 GiB/s]

parse/heavy/full        time:   [1.1465 ms 1.1474 ms 1.1487 ms]
                        thrpt:  [1015.5 MiB/s 1016.7 MiB/s 1017.5 MiB/s]
```

**big (превышает L3-кэш — видно давление аллокатора):**

```
parse/plain/full        time:   [105.96 ms 106.08 ms 106.21 ms]
                        thrpt:  [2.5762 GiB/s 2.5793 GiB/s 2.5822 GiB/s]

parse/hot/full          time:   [97.604 ms 97.740 ms 97.885 ms]
                        thrpt:  [770.27 MiB/s 771.40 MiB/s 772.48 MiB/s]

parse/heavy/full        time:   [176.51 ms 176.74 ms 176.98 ms]
                        thrpt:  [659.14 MiB/s 660.03 MiB/s 660.89 MiB/s]
```

> Пропускная способность `plain` остаётся стабильной при масштабировании,
> потому что парсер эмитирует почти ноль спанов (2 штуки) и давление `Vec`
> пренебрежимо мало. `hot` и `heavy` падают на ~30–35% когда спан-`Vec`-ы
> вылезают из кэша — см.
> [Известные характеристики производительности](#известные-характеристики-производительности).

---

### nightly — `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_parse --features avx2`

**small (помещается в кэш):**

```
parse/plain/full        time:   [648.73 µs 649.08 µs 649.42 µs]
                        thrpt:  [4.2131 GiB/s 4.2152 GiB/s 4.2175 GiB/s]

parse/hot/full          time:   [524.88 µs 525.16 µs 525.50 µs]
                        thrpt:  [1.4012 GiB/s 1.4021 GiB/s 1.4028 GiB/s]

parse/heavy/full        time:   [981.25 µs 982.51 µs 983.97 µs]
                        thrpt:  [1.1578 GiB/s 1.1595 GiB/s 1.1610 GiB/s]
```

**big (превышает L3-кэш):**

```
parse/plain/full        time:   [60.957 ms 60.983 ms 61.014 ms]
                        thrpt:  [4.4843 GiB/s 4.4865 GiB/s 4.4885 GiB/s]

parse/hot/full          time:   [86.182 ms 86.249 ms 86.344 ms]
                        thrpt:  [873.22 MiB/s 874.18 MiB/s 874.87 MiB/s]

parse/heavy/full        time:   [158.66 ms 158.78 ms 158.89 ms]
                        thrpt:  [734.20 MiB/s 734.72 MiB/s 735.24 MiB/s]
```

---

## Результаты — standalone (`meon-md_standalone`)

Каждая строка выводит счётчики `full` vs `standalone`. По замыслу они могут
расходиться: standalone-сканирование не имеет контекста ограждений и экранирования
(см.
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE.md#12-standalone-iterators)).

### stable — `cargo bench --bench meon-md_standalone`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0
standalone/plain/find_codes             time: [459.19 µs 459.70 µs 460.24 µs]   thrpt: [5.9448 GiB/s 5.9518 GiB/s 5.9585 GiB/s]

find_italics       full=       0  standalone=       0
standalone/plain/find_italics           time: [462.13 µs 462.49 µs 462.89 µs]   thrpt: [5.9108 GiB/s 5.9159 GiB/s 5.9205 GiB/s]

find_bolds         full=       0  standalone=       0
standalone/plain/find_bolds             time: [458.27 µs 458.98 µs 459.96 µs]   thrpt: [5.9484 GiB/s 5.9611 GiB/s 5.9703 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/plain/find_bold_italics      time: [459.40 µs 459.78 µs 460.21 µs]   thrpt: [5.9452 GiB/s 5.9508 GiB/s 5.9556 GiB/s]

find_autolinks     full=       0  standalone=       0
standalone/plain/find_autolinks         time: [450.55 µs 451.09 µs 451.83 µs]   thrpt: [6.0555 GiB/s 6.0653 GiB/s 6.0726 GiB/s]

find_links         full=       0  standalone=       0
standalone/plain/find_links             time: [468.88 µs 469.27 µs 469.72 µs]   thrpt: [5.8248 GiB/s 5.8304 GiB/s 5.8353 GiB/s]

find_headings      full=       0  standalone=       0
standalone/plain/find_headings          time: [253.63 µs 254.52 µs 255.36 µs]   thrpt: [10.714 GiB/s 10.750 GiB/s 10.787 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/plain/find_thematic_breaks  time: [280.60 µs 280.76 µs 280.93 µs]   thrpt: [9.7393 GiB/s 9.7450 GiB/s 9.7506 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/plain/find_fenced_codes      time: [276.86 µs 277.37 µs 278.07 µs]   thrpt: [9.8393 GiB/s 9.8642 GiB/s 9.8822 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/plain/find_blockquotes       time: [252.08 µs 252.30 µs 252.55 µs]   thrpt: [10.834 GiB/s 10.844 GiB/s 10.854 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/plain/find_bullet_items      time: [269.48 µs 269.77 µs 270.10 µs]   thrpt: [10.130 GiB/s 10.142 GiB/s 10.153 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/plain/find_ordered_items     time: [284.16 µs 284.85 µs 285.62 µs]   thrpt: [9.5792 GiB/s 9.6051 GiB/s 9.6284 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000
standalone/hot/find_codes               time: [236.40 µs 236.72 µs 237.12 µs]   thrpt: [3.1052 GiB/s 3.1105 GiB/s 3.1147 GiB/s]

find_italics       full=    5000  standalone=    5000
standalone/hot/find_italics             time: [285.66 µs 285.81 µs 285.94 µs]   thrpt: [2.5750 GiB/s 2.5762 GiB/s 2.5776 GiB/s]

find_bolds         full=    5000  standalone=    5000
standalone/hot/find_bolds               time: [285.83 µs 286.02 µs 286.24 µs]   thrpt: [2.5723 GiB/s 2.5743 GiB/s 2.5761 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/hot/find_bold_italics        time: [269.97 µs 270.34 µs 270.73 µs]   thrpt: [2.7197 GiB/s 2.7237 GiB/s 2.7273 GiB/s]

find_autolinks     full=    5000  standalone=    5000
standalone/hot/find_autolinks           time: [238.92 µs 239.24 µs 239.56 µs]   thrpt: [3.0735 GiB/s 3.0777 GiB/s 3.0818 GiB/s]

find_links         full=    5000  standalone=    5000
standalone/hot/find_links               time: [270.64 µs 270.78 µs 270.91 µs]   thrpt: [2.7179 GiB/s 2.7192 GiB/s 2.7206 GiB/s]

find_headings      full=    5000  standalone=    5000
standalone/hot/find_headings            time: [119.56 µs 119.67 µs 119.77 µs]   thrpt: [6.1479 GiB/s 6.1530 GiB/s 6.1583 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/hot/find_thematic_breaks    time: [125.09 µs 125.15 µs 125.22 µs]   thrpt: [5.8801 GiB/s 5.8832 GiB/s 5.8860 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/hot/find_fenced_codes        time: [125.78 µs 125.84 µs 125.91 µs]   thrpt: [5.8481 GiB/s 5.8511 GiB/s 5.8537 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/hot/find_blockquotes         time: [117.63 µs 117.69 µs 117.77 µs]   thrpt: [6.2522 GiB/s 6.2561 GiB/s 6.2597 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/hot/find_bullet_items        time: [124.88 µs 124.98 µs 125.08 µs]   thrpt: [5.8867 GiB/s 5.8914 GiB/s 5.8960 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/hot/find_ordered_items       time: [129.16 µs 129.22 µs 129.30 µs]   thrpt: [5.6945 GiB/s 5.6980 GiB/s 5.7007 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000
standalone/heavy/find_codes             time: [522.74 µs 523.80 µs 524.92 µs]   thrpt: [2.1703 GiB/s 2.1749 GiB/s 2.1793 GiB/s]

find_italics       full=    8000  standalone=    8000
standalone/heavy/find_italics           time: [660.67 µs 662.47 µs 664.32 µs]   thrpt: [1.7149 GiB/s 1.7197 GiB/s 1.7243 GiB/s]

find_bolds         full=   10000  standalone=   10000
standalone/heavy/find_bolds             time: [666.29 µs 668.17 µs 669.67 µs]   thrpt: [1.7011 GiB/s 1.7050 GiB/s 1.7098 GiB/s]

find_bold_italics  full=    6000  standalone=    6000
standalone/heavy/find_bold_italics      time: [660.76 µs 662.30 µs 663.54 µs]   thrpt: [1.7169 GiB/s 1.7201 GiB/s 1.7241 GiB/s]

find_autolinks     full=    4000  standalone=    4000
standalone/heavy/find_autolinks         time: [417.47 µs 418.30 µs 419.39 µs]   thrpt: [2.7164 GiB/s 2.7234 GiB/s 2.7288 GiB/s]

find_links         full=    6000  standalone=    6000
standalone/heavy/find_links             time: [480.19 µs 481.28 µs 482.55 µs]   thrpt: [2.3608 GiB/s 2.3671 GiB/s 2.3724 GiB/s]

find_headings      full=    2000  standalone=    2000
standalone/heavy/find_headings          time: [221.27 µs 221.33 µs 221.40 µs]   thrpt: [5.1454 GiB/s 5.1470 GiB/s 5.1485 GiB/s]

find_thematic_breaks full=  2000  standalone=    2000
standalone/heavy/find_thematic_breaks  time: [233.49 µs 233.55 µs 233.62 µs]   thrpt: [4.8763 GiB/s 4.8777 GiB/s 4.8790 GiB/s]

find_fenced_codes  full=    2000  standalone=    2000
standalone/heavy/find_fenced_codes      time: [221.54 µs 221.59 µs 221.65 µs]   thrpt: [5.1397 GiB/s 5.1410 GiB/s 5.1423 GiB/s]

find_blockquotes   full=    2000  standalone=    2000
standalone/heavy/find_blockquotes       time: [205.28 µs 205.35 µs 205.41 µs]   thrpt: [5.5460 GiB/s 5.5478 GiB/s 5.5496 GiB/s]

find_bullet_items  full=    6000  standalone=    6000
standalone/heavy/find_bullet_items      time: [215.46 µs 215.53 µs 215.61 µs]   thrpt: [5.2836 GiB/s 5.2855 GiB/s 5.2873 GiB/s]

find_ordered_items full=    4000  standalone=    4000
standalone/heavy/find_ordered_items     time: [233.19 µs 233.24 µs 233.28 µs]   thrpt: [4.8834 GiB/s 4.8843 GiB/s 4.8853 GiB/s]
```

</details>

---

### nightly — `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-md_standalone --features avx2`

<details>
<summary>plain</summary>

```
find_codes         full=       0  standalone=       0
standalone/plain/find_codes             time: [325.93 µs 326.09 µs 326.25 µs]   thrpt: [8.3862 GiB/s 8.3905 GiB/s 8.3946 GiB/s]

find_italics       full=       0  standalone=       0
standalone/plain/find_italics           time: [324.03 µs 324.21 µs 324.40 µs]   thrpt: [8.4343 GiB/s 8.4391 GiB/s 8.4438 GiB/s]

find_bolds         full=       0  standalone=       0
standalone/plain/find_bolds             time: [324.04 µs 324.26 µs 324.52 µs]   thrpt: [8.4310 GiB/s 8.4378 GiB/s 8.4436 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/plain/find_bold_italics      time: [323.55 µs 323.75 µs 323.99 µs]   thrpt: [8.4448 GiB/s 8.4511 GiB/s 8.4563 GiB/s]

find_autolinks     full=       0  standalone=       0
standalone/plain/find_autolinks         time: [324.64 µs 324.95 µs 325.27 µs]   thrpt: [8.4116 GiB/s 8.4199 GiB/s 8.4278 GiB/s]

find_links         full=       0  standalone=       0
standalone/plain/find_links             time: [327.63 µs 328.02 µs 328.41 µs]   thrpt: [8.3311 GiB/s 8.3411 GiB/s 8.3510 GiB/s]

find_headings      full=       0  standalone=       0
standalone/plain/find_headings          time: [250.10 µs 250.39 µs 250.64 µs]   thrpt: [10.916 GiB/s 10.927 GiB/s 10.940 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/plain/find_thematic_breaks  time: [272.07 µs 272.19 µs 272.32 µs]   thrpt: [10.047 GiB/s 10.052 GiB/s 10.056 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/plain/find_fenced_codes      time: [267.25 µs 267.34 µs 267.44 µs]   thrpt: [10.230 GiB/s 10.234 GiB/s 10.238 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/plain/find_blockquotes       time: [246.24 µs 246.49 µs 246.76 µs]   thrpt: [11.088 GiB/s 11.100 GiB/s 11.111 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/plain/find_bullet_items      time: [263.03 µs 263.21 µs 263.39 µs]   thrpt: [10.388 GiB/s 10.395 GiB/s 10.402 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/plain/find_ordered_items     time: [279.40 µs 279.60 µs 279.83 µs]   thrpt: [9.7777 GiB/s 9.7855 GiB/s 9.7926 GiB/s]
```

</details>

<details>
<summary>hot</summary>

```
find_codes         full=    5000  standalone=    5000
standalone/hot/find_codes               time: [179.99 µs 180.04 µs 180.10 µs]   thrpt: [4.0883 GiB/s 4.0896 GiB/s 4.0907 GiB/s]

find_italics       full=    5000  standalone=    5000
standalone/hot/find_italics             time: [231.64 µs 231.75 µs 231.88 µs]   thrpt: [3.1754 GiB/s 3.1772 GiB/s 3.1787 GiB/s]

find_bolds         full=    5000  standalone=    5000
standalone/hot/find_bolds               time: [229.37 µs 229.48 µs 229.62 µs]   thrpt: [3.2066 GiB/s 3.2085 GiB/s 3.2101 GiB/s]

find_bold_italics  full=       0  standalone=       0
standalone/hot/find_bold_italics        time: [222.71 µs 222.87 µs 223.05 µs]   thrpt: [3.3010 GiB/s 3.3038 GiB/s 3.3062 GiB/s]

find_autolinks     full=    5000  standalone=    5000
standalone/hot/find_autolinks           time: [204.88 µs 205.01 µs 205.16 µs]   thrpt: [3.5889 GiB/s 3.5915 GiB/s 3.5938 GiB/s]

find_links         full=    5000  standalone=    5000
standalone/hot/find_links               time: [204.65 µs 204.70 µs 204.75 µs]   thrpt: [3.5961 GiB/s 3.5970 GiB/s 3.5978 GiB/s]

find_headings      full=    5000  standalone=    5000
standalone/hot/find_headings            time: [109.06 µs 109.12 µs 109.18 µs]   thrpt: [6.7442 GiB/s 6.7478 GiB/s 6.7512 GiB/s]

find_thematic_breaks full=     0  standalone=       0
standalone/hot/find_thematic_breaks    time: [118.63 µs 118.66 µs 118.69 µs]   thrpt: [6.2034 GiB/s 6.2049 GiB/s 6.2067 GiB/s]

find_fenced_codes  full=       0  standalone=       0
standalone/hot/find_fenced_codes        time: [117.25 µs 117.31 µs 117.38 µs]   thrpt: [6.2730 GiB/s 6.2768 GiB/s 6.2800 GiB/s]

find_blockquotes   full=       0  standalone=       0
standalone/hot/find_blockquotes         time: [109.06 µs 109.09 µs 109.13 µs]   thrpt: [6.7469 GiB/s 6.7492 GiB/s 6.7515 GiB/s]

find_bullet_items  full=       0  standalone=       0
standalone/hot/find_bullet_items        time: [118.78 µs 118.83 µs 118.90 µs]   thrpt: [6.1928 GiB/s 6.1961 GiB/s 6.1991 GiB/s]

find_ordered_items full=       0  standalone=       0
standalone/hot/find_ordered_items       time: [121.86 µs 121.90 µs 121.95 µs]   thrpt: [6.0379 GiB/s 6.0401 GiB/s 6.0423 GiB/s]
```

</details>

<details>
<summary>heavy</summary>

```
find_codes         full=   10000  standalone=   10000
standalone/heavy/find_codes             time: [383.05 µs 383.45 µs 383.85 µs]   thrpt: [2.9679 GiB/s 2.9710 GiB/s 2.9741 GiB/s]

find_italics       full=    8000  standalone=    8000
standalone/heavy/find_italics           time: [492.48 µs 492.59 µs 492.69 µs]   thrpt: [2.3122 GiB/s 2.3127 GiB/s 2.3132 GiB/s]

find_bolds         full=   10000  standalone=   10000
standalone/heavy/find_bolds             time: [493.13 µs 493.43 µs 493.96 µs]   thrpt: [2.3063 GiB/s 2.3088 GiB/s 2.3102 GiB/s]

find_bold_italics  full=    6000  standalone=    6000
standalone/heavy/find_bold_italics      time: [491.63 µs 491.73 µs 491.83 µs]   thrpt: [2.3163 GiB/s 2.3168 GiB/s 2.3172 GiB/s]

find_autolinks     full=    4000  standalone=    4000
standalone/heavy/find_autolinks         time: [308.50 µs 308.71 µs 308.90 µs]   thrpt: [3.6879 GiB/s 3.6903 GiB/s 3.6927 GiB/s]

find_links         full=    6000  standalone=    6000
standalone/heavy/find_links             time: [361.01 µs 361.30 µs 361.66 µs]   thrpt: [3.1499 GiB/s 3.1531 GiB/s 3.1557 GiB/s]

find_headings      full=    2000  standalone=    2000
standalone/heavy/find_headings          time: [203.22 µs 203.37 µs 203.51 µs]   thrpt: [5.5978 GiB/s 5.6015 GiB/s 5.6057 GiB/s]

find_thematic_breaks full=  2000  standalone=    2000
standalone/heavy/find_thematic_breaks  time: [217.69 µs 217.79 µs 217.87 µs]   thrpt: [5.2287 GiB/s 5.2309 GiB/s 5.2333 GiB/s]

find_fenced_codes  full=    2000  standalone=    2000
standalone/heavy/find_fenced_codes      time: [221.54 µs 221.59 µs 221.65 µs]   thrpt: [5.1397 GiB/s 5.1410 GiB/s 5.1423 GiB/s]

find_blockquotes   full=    2000  standalone=    2000
standalone/heavy/find_blockquotes       time: [205.28 µs 205.35 µs 205.41 µs]   thrpt: [5.5460 GiB/s 5.5478 GiB/s 5.5496 GiB/s]

find_bullet_items  full=    6000  standalone=    6000
standalone/heavy/find_bullet_items      time: [215.46 µs 215.53 µs 215.61 µs]   thrpt: [5.2836 GiB/s 5.2855 GiB/s 5.2873 GiB/s]

find_ordered_items full=    4000  standalone=    4000
standalone/heavy/find_ordered_items     time: [233.19 µs 233.24 µs 233.28 µs]   thrpt: [4.8834 GiB/s 4.8843 GiB/s 4.8853 GiB/s]
```

</details>

---

## Как читать числа

- `thrpt` (GiB/s) — главная метрика; она уже учитывает размер корпуса.
- Сравнивайте число только с *тем же корпусом* на *другом билде*
  (scalar vs AVX2) или с предыдущим коммитом на том же железе.
- `plain` самый быстрый (меньше работы); `heavy` самый медленный (больше
  элементов). Отчёт о составе объясняет *почему*.
- `plain` стабилен при масштабировании — он эмитирует почти ноль спанов.
  `hot`/`heavy` падают на ~30–35% при большом масштабе из-за давления `Vec` —
  см. [Известные характеристики производительности](#известные-характеристики-производительности).
- Criterion пишет HTML-отчёты в `target/criterion/`; блок `change:` появляется
  автоматически при повторном прогоне и является настоящим сигналом регрессии.
