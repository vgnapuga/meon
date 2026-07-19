# meon — Бенчмарки

[**EN**](https://github.com/vgnapuga/meon/blob/main/benches/README.md) | RU

Бенчмарки пропускной способности для грамматик [`meon-md`](https://github.com/vgnapuga/meon/blob/main/meon-md/README.md)
и [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README.md),
построенных на движке [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README.md).
Цель — отслеживать производительность движка между изменениями и при переключении
feature-флагов и ставить его плоские векторы спанов рядом с парсерами,
производящими другие формы вывода (см.
[Честность измерений](#честность-измерений)).

| Бенч                  | Измеряет                                                                    |
|-----------------------|-----------------------------------------------------------------------------|
| `meon-md_parse`       | `MarkdownParser::parse` — полный однопроходной парс.                        |
| `meon-md_standalone`  | Итераторы `find_*` — один вид элементов, без контекста; плюс постройка карты `context()` и контекстные варианты `find_context_*` (тёплые и холодные). |
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
| `numbers` | Плоский массив чисел / bool / null.                                            | Скан скаляров. meon выдаёт векторы спанов; валидирующие парсят каждое число. |
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
  `meon-json_parse` измеряют *этот* движок на *этих* корпусах — «регрессировало
  ли моё изменение?» и «насколько помогает AVX2?».
- **Cross-parser сравнения — архитектурные.** `meon-md` возвращает плоские
  векторы спанов для *подмножества* Markdown (без AST, резолва ссылок, рендера);
  `meon-json` — *структурный ридер* (без валидации, парсинга чисел и
  разэкранирования строк). Сравнения — против `pulldown-cmark` / `comrak`
  ([***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md))
  и против `simd-json` / `sonic-rs`
  ([***JSON_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE_RU.md))
  — поданы там как разные задачи с разными формами вывода: векторы спанов с
  одной стороны, поток событий, AST, tape или владеющее значение с другой.
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

---

## Микроархитектура

Аппаратные счётчики полного прохода `meon-md_parse` по каждому корпусу, сняты
на железе выше (`perf stat`, 10 прогонов, user-space счётчики, stable-сборка,
`--profile-time 10`). Каждая ячейка читается как `small -> big`:

| Корпус  | insn/cycle   | branch-misses  | cache-misses   |
|---------|--------------|----------------|----------------|
| `plain` | 4.94 -> 4.82 | 0.11% -> 0.09% | 1.61% -> 1.58% |
| `hot`   | 4.55 -> 4.27 | 0.08% -> 0.09% | 4.20% -> 3.12% |
| `heavy` | 4.11 -> 3.85 | 0.11% -> 0.12% | 6.74% -> 2.88% |

IPC держится на 3.9-4.9 при branch-misses около 0.1%, а доля cache-misses не
растёт при увеличении входа в ~100 раз от `small` к `big` — рабочее множество
плоских векторов спанов остаётся кэш-резидентным.

<details>
<summary>small</summary>

```
 Performance counter stats for 'cargo bench --bench meon-md_parse -- plain/full --profile-time 10' (10 runs):

         10 188,03 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,08% )
    41 703 907 951      cycles:u                         #    4,093 GHz                         ( +-  0,09% )  (83,32%)
   206 061 678 743      instructions:u                   #    4,94  insn per cycle              ( +-  0,10% )  (83,34%)
    31 784 289 849      branches:u                       #    3,120 G/sec                       ( +-  0,10% )  (83,36%)
        34 402 750      branch-misses:u                  #    0,11% of all branches             ( +-  0,09% )  (83,35%)
     1 772 186 919      cache-references:u               #  173,948 M/sec                       ( +-  0,11% )  (83,33%)
        28 472 216      cache-misses:u                   #    1,61% of all cache refs           ( +-  2,26% )  (83,32%)

           10,1941 +- 0,0103 seconds time elapsed  ( +-  0,10% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 197,21 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,09% )
    43 358 916 187      cycles:u                         #    4,252 GHz                         ( +-  0,07% )  (83,31%)
   197 334 384 022      instructions:u                   #    4,55  insn per cycle              ( +-  0,07% )  (83,35%)
    38 453 951 651      branches:u                       #    3,771 G/sec                       ( +-  0,07% )  (83,35%)
        31 655 793      branch-misses:u                  #    0,08% of all branches             ( +-  0,15% )  (83,35%)
       771 926 269      cache-references:u               #   75,700 M/sec                       ( +-  0,07% )  (83,34%)
        32 419 197      cache-misses:u                   #    4,20% of all cache refs           ( +-  0,75% )  (83,33%)

          10,20385 +- 0,00828 seconds time elapsed  ( +-  0,08% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 200,95 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,10% )
    43 380 829 468      cycles:u                         #    4,253 GHz                         ( +-  0,08% )  (83,31%)
   178 171 172 323      instructions:u                   #    4,11  insn per cycle              ( +-  0,10% )  (83,36%)
    36 240 505 684      branches:u                       #    3,553 G/sec                       ( +-  0,10% )  (83,35%)
        38 582 525      branch-misses:u                  #    0,11% of all branches             ( +-  0,21% )  (83,33%)
       692 203 903      cache-references:u               #   67,857 M/sec                       ( +-  0,09% )  (83,34%)
        46 647 503      cache-misses:u                   #    6,74% of all cache refs           ( +-  1,26% )  (83,34%)

           10,2061 +- 0,0101 seconds time elapsed  ( +-  0,10% )
```

</details>

<details>
<summary>big</summary>

```
 Performance counter stats for 'cargo bench --bench meon-md_parse -- plain/full --profile-time 10' (10 runs):

         10 839,57 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,07% )
    44 748 388 452      cycles:u                         #    4,128 GHz                         ( +-  0,07% )  (83,33%)
   215 463 976 982      instructions:u                   #    4,82  insn per cycle              ( +-  0,30% )  (83,34%)
    33 406 712 411      branches:u                       #    3,082 G/sec                       ( +-  0,29% )  (83,35%)
        30 402 477      branch-misses:u                  #    0,09% of all branches             ( +-  0,34% )  (83,34%)
     1 856 418 129      cache-references:u               #  171,263 M/sec                       ( +-  0,35% )  (83,33%)
        29 363 581      cache-misses:u                   #    1,58% of all cache refs           ( +-  2,97% )  (83,34%)

          10,84521 +- 0,00770 seconds time elapsed  ( +-  0,07% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- hot/full --profile-time 10' (10 runs):

         10 831,93 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,28% )
    32 349 636 825      cycles:u                         #    2,987 GHz                         ( +-  0,28% )  (83,34%)
   138 012 462 194      instructions:u                   #    4,27  insn per cycle              ( +-  0,27% )  (83,33%)
    26 822 349 966      branches:u                       #    2,476 G/sec                       ( +-  0,27% )  (83,35%)
        25 188 136      branch-misses:u                  #    0,09% of all branches             ( +-  0,26% )  (83,34%)
       673 691 545      cache-references:u               #   62,195 M/sec                       ( +-  0,30% )  (83,33%)
        21 031 825      cache-misses:u                   #    3,12% of all cache refs           ( +-  2,04% )  (83,33%)

           10,8402 +- 0,0289 seconds time elapsed  ( +-  0,27% )

 Performance counter stats for 'cargo bench --bench meon-md_parse -- heavy/full --profile-time 10' (10 runs):

         10 852,56 msec task-clock:u                     #    0,999 CPUs utilized               ( +-  0,09% )
    33 022 729 384      cycles:u                         #    3,043 GHz                         ( +-  0,05% )  (83,35%)
   127 002 621 975      instructions:u                   #    3,85  insn per cycle              ( +-  0,01% )  (83,34%)
    25 708 883 847      branches:u                       #    2,369 G/sec                       ( +-  0,01% )  (83,36%)
        31 236 788      branch-misses:u                  #    0,12% of all branches             ( +-  0,10% )  (83,33%)
       844 217 588      cache-references:u               #   77,790 M/sec                       ( +-  0,09% )  (83,32%)
        24 276 598      cache-misses:u                   #    2,88% of all cache refs           ( +-  1,57% )  (83,32%)

          10,86128 +- 0,00872 seconds time elapsed  ( +-  0,08% )
```

</details>
