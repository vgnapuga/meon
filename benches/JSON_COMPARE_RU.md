# meon-json — Сравнение парсеров

[**EN**](https://github.com/vgnapuga/meon/blob/main/benches/JSON_COMPARE.md) | RU

Пропускная способность [`meon-json`](https://github.com/vgnapuga/meon/blob/main/meon-json/README_RU.md)
(построен на движке [`meon`](https://github.com/vgnapuga/meon/blob/main/meon/README_RU.md))
рядом с двумя валидирующими JSON-парсерами, на тех же корпусах, что и
внутридвижковые бенчмарки.

> **Четыре парсера — две разные задачи.** `meon-json` по своей сути —
> **структурный читатель**: он разбирает JSON в плоские векторы спанов — без
> валидации, без парсинга чисел, без разэкранирования строк. `simd-json` и
> `sonic-rs` — валидирующие парсеры, которые материализуют tape / владеющее
> значение: они парсят каждое число и разэкранируют каждую строку. Разрыв в
> пропускной способности — это разница между этими задачами. `Throughput::Bytes`
> измеряет, как быстро потребляется вход, поскольку все четыре производят разные
> вещи.

* **meon**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon/README_RU.md)
  * [***crates.io***](https://crates.io/crates/meon)
* **meon-macros**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-macros/README_RU.md)
  * [***crates.io***](https://crates.io/crates/meon-macros)
* **meon-md**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-md/README_RU.md)
  * [***crates.io***](https://crates.io/crates/meon-md)
* **meon-json**
  * [***GitHub***](https://github.com/vgnapuga/meon/blob/main/meon-json/README_RU.md)
  * [***crates.io***](https://crates.io/crates/meon-json)

* [***CHANGELOG.md***](https://github.com/vgnapuga/meon/blob/main/CHANGELOG.md)
* [***ARCHITECTURE.md***](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE_RU.md)
* [***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)
  * [***MD_COMPARE.md***](https://github.com/vgnapuga/meon/blob/main/benches/MD_COMPARE_RU.md)
  * ***JSON_COMPARE.md***    <--
* [***FUZZING.md***](https://github.com/vgnapuga/meon/blob/main/fuzz/README_RU.md)

---

## Что измеряется

Один бинарь, `meon-json_compare`. На каждый корпус (`numbers` / `objects` /
`nested`) — четыре парсера на идентичном входе, каждый под `black_box`:

| Линия             | Вызов                               | Что делает                                                                          |
|-------------------|-------------------------------------|-------------------------------------------------------------------------------------|
| `meon-structural` | `JsonParser::parse`                 | Плоская таблица спанов. Без валидации, без парсинга чисел, без разэкранирования.     |
| `meon-typed`      | `parse` + `type_scalars`            | + классификация скаляров по первому байту. По-прежнему без парсинга чисел и разэкранирования. |
| `simd-json`       | `simd_json::to_tape`                | Stage 1 + Stage 2 за один проход: структура + парсинг/валидация чисел + разэкранирование. |
| `sonic-rs`        | `sonic_rs::from_slice::<Value>`     | Полный разбор во владеющее `Value` (валидирует, парсит числа, разэкранирует).        |

`simd-json` мутирует свой входной буфер на месте (разэкранирование строк),
поэтому ему отдаётся свежая копия на каждой итерации; эта копия — часть setup
`iter_batched` и **не** хронометрируется. Остальные три читают исходные
неизменяемые байты.

Тот же поэлементный отчёт о составе (структурные + типизированные счётчики), что
и у внутридвижковых бенчмарков, печатается до хронометража.

---

## Две разные задачи

- **Разные выходы.** `meon-structural` выдаёт векторы спанов и ничего не
  материализует. `meon-typed` добавляет лишь классификацию по первому байту.
  `simd-json` и `sonic-rs` валидируют, парсят каждое число в значение и
  разэкранируют каждую строку. Разрыв в пропускной способности — это стоимость
  той материализации. Линия с той же задачей тоже парсила бы числа в значения и
  разэкранировала бы строки — **ни одна линия meon этого не делает**, так что
  даже `meon-typed` решает другую задачу, нежели tape или владеющее значение.

- **Читатель, а не валидатор — намеренно.** `meon-json` не отвергает невалидный
  JSON; он сообщает о структуре, которую увидел. `simd-json` и `sonic-rs`
  валидируют и падают с ошибкой на некорректном входе. Сравнение не
  «один-в-один» по гарантиям.

- **`meon-typed` — это классификация по первому байту, а не валидация чисел.**
  Он маршрутизирует скаляр по первому байту (`1abc` типизируется как число); он
  никогда не проверяет остаток токена, не парсит числовое значение и не
  декодирует строку.

- **Паритет флагов сборки / SIMD.** meon использует AVX2 только под
  `--features avx2` + `RUSTFLAGS="-C target-cpu=native"`; на stable он идёт по
  скалярному пути SWAR. `simd-json` и `sonic-rs` сами определяют SIMD в рантайме
  и используют его на подходящем железе независимо от флага meon. Скалярная
  строка meon рядом с SIMD-компараторами — не «один-в-один» SIMD-сравнение;
  каждый блок результатов указывает, под какой сборкой meon он снят.

- **Формы вывода различаются.** SoA-спаны против tape против владеющего `Value`.
  `Throughput::Bytes` нормирует по размеру входа — он отвечает на вопрос «как
  быстро потребляется вход», поскольку все четыре производят разные вещи.

- **Сквозная стоимость и скрытое преимущество meon.** В хронометраж входят
  собственные аллокации каждого парсера (`Vec`-и meon, tape simd-json, значение
  sonic-rs). Копия, которая нужна `simd-json` (чтобы сохранить оригинал, ведь он
  разэкранирует на месте), из хронометража исключена — то есть zero-copy,
  немутирующее чтение meon здесь не засчитано. Если вашему сценарию нужно
  сохранить исходные байты, добавьте эту копию к стоимости `simd-json`.

- **Смещение корпуса.** Корпуса синтетические. Корпус `numbers` максимизирует
  разрыв — meon никогда не парсит число, тогда как валидирующие парсеры парсят и
  валидируют каждое — поэтому читайте каждый корпус по отдельности, а не как один
  заголовок.

---

## Запуск

Внутри `nix develop`:

```sh
# Stable, скалярный путь SWAR у meon (simd-json / sonic-rs используют рантайм-SIMD):
cargo bench --bench meon-json_compare

# Nightly, путь AVX2 у meon, заточенный под CPU хоста:
RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2
```

`simd-json` и `sonic-rs` определяют и используют SIMD в рантайме; никакой
Cargo-фичи для них не нужно. Только путь AVX2 у meon скрыт за `--features avx2`.

Железо и параметры Criterion общие с внутридвижковыми бенчмарками — см.
*Test hardware* в
[***BENCHMARKS.md***](https://github.com/vgnapuga/meon/blob/main/benches/README_RU.md)
и параметры в `benches/benches/docs_json.rs`.

---

## Корпуса

Каждый корпус — один валидный JSON-массив верхнего уровня, масштабируемый через
`COUNT` (`benches/benches/docs_json.rs`). Прогоны `small` и `big` отличаются
только значением `COUNT`.

| Корпус    | Форма                                                                                       | Что нагружает                                                              |
|-----------|---------------------------------------------------------------------------------------------|---------------------------------------------------------------------------|
| `numbers` | Плоский массив чисел / булевых / null.                                                       | Сканирование скаляров. meon выдаёт векторы спанов; валидаторы парсят каждое число. |
| `objects` | Массив плоских объектов с разнотипными полями (`id`/`name`/`active`/...).                    | Члены, ключи, типизированные скаляры. Типичная API-нагрузка.              |
| `nested`  | Массив умеренно вложенных объектов (объекты-в-объектах, малые массивы, экранированная строка). | Единый стек вложенности и правило строк.                                  |

> **Замечание о синтетических данных.** Все три корпуса генерируются программно,
> с однородной, предсказуемой структурой. Реальный JSON обычно менее регулярен —
> воспринимайте цифры как демонстрацию архитектурной разницы, а не как ожидаемую
> продакшн-пропускную способность.

### Состав корпусов

**small:**

```
┌─ corpus: numbers
│  size:            1.90 MiB  (1989441 bytes)
│  structural:         1     (0.0 per KiB)
│
│      objects:         0      arrays:         1     strings:         0
│      members:         0     scalars:         0       loose:         1
│  typed: nums:    150000       trues:     50000      falses:     50000     nulls:     50000
└─
┌─ corpus: objects
│  size:            1.39 MiB  (1456671 bytes)
│  structural:    240001     (168.7 per KiB)
│
│      objects:     20000      arrays:         1     strings:    120000
│      members:    100000     scalars:         0       loose:         1
│  typed: nums:     40000       trues:     10000      falses:     10000     nulls:     20000
└─
┌─ corpus: nested
│  size:            1.13 MiB  (1184451 bytes)
│  structural:    290001     (250.7 per KiB)
│
│      objects:     50000      arrays:     10001     strings:    130000
│      members:    100000     scalars:         0       loose:         1
│  typed: nums:     40000       trues:     10000      falses:         0     nulls:         0
└─
```

**big:**

```
┌─ corpus: numbers
│  size:          218.34 MiB  (228944439 bytes)
│  structural:         1     (0.0 per KiB)
│
│      objects:         0      arrays:         1     strings:         0
│      members:         0     scalars:         0       loose:         1
│  typed: nums:  15000000       trues:   5000000      falses:   5000000     nulls:   5000000
└─
┌─ corpus: objects
│  size:          150.36 MiB  (157666671 bytes)
│  structural:  24000001     (155.9 per KiB)
│
│      objects:   2000000      arrays:         1     strings:  12000000
│      members:  10000000     scalars:         0       loose:         1
│  typed: nums:   4000000       trues:   1000000      falses:   1000000     nulls:   2000000
└─
┌─ corpus: nested
│  size:          122.49 MiB  (128444451 bytes)
│  structural:  29000001     (231.2 per KiB)
│
│      objects:   5000000      arrays:   1000001     strings:  13000000
│      members:  10000000     scalars:         0       loose:         1
│  typed: nums:   4000000       trues:   1000000      falses:         0     nulls:         0
└─
```

---

## Результаты

> Пропускная способность (`thrpt`) — главное. Сравнивайте ячейку только с тем же
> корпусом в том же блоке сборки. Каждая ячейка — тройка Criterion `time` /
> `thrpt` (низ / медиана / верх).

### stable - `cargo bench --bench meon-json_compare`

**small:**

| Корпус    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [2.3261 ms 2.3282 ms 2.3303 ms] thrpt:  [814.18 MiB/s 814.92 MiB/s 815.64 MiB/s] | time:   [4.8090 ms 4.8169 ms 4.8248 ms] thrpt:  [393.24 MiB/s 393.88 MiB/s 394.52 MiB/s] | time:   [8.1397 ms 8.1854 ms 8.2325 ms] thrpt:  [230.46 MiB/s 231.79 MiB/s 233.09 MiB/s] | time:   [2.7498 ms 2.7667 ms 2.7842 ms] thrpt:  [681.44 MiB/s 685.75 MiB/s 689.98 MiB/s] |
| `objects` | time:   [3.8821 ms 3.8903 ms 3.8987 ms] thrpt:  [356.33 MiB/s 357.09 MiB/s 357.85 MiB/s] | time:   [5.1094 ms 5.1192 ms 5.1290 ms] thrpt:  [270.85 MiB/s 271.37 MiB/s 271.89 MiB/s] | time:   [1.9563 ms 1.9619 ms 1.9681 ms] thrpt:  [705.84 MiB/s 708.09 MiB/s 710.11 MiB/s] | time:   [1.7473 ms 1.7530 ms 1.7607 ms] thrpt:  [788.99 MiB/s 792.45 MiB/s 795.03 MiB/s] |
| `nested`  | time:   [4.6466 ms 4.6579 ms 4.6714 ms] thrpt:  [241.81 MiB/s 242.51 MiB/s 243.10 MiB/s] | time:   [5.6025 ms 5.6145 ms 5.6275 ms] thrpt:  [200.72 MiB/s 201.19 MiB/s 201.62 MiB/s] | time:   [2.1602 ms 2.1631 ms 2.1669 ms] thrpt:  [521.29 MiB/s 522.21 MiB/s 522.90 MiB/s] | time:   [2.2674 ms 2.2686 ms 2.2698 ms] thrpt:  [497.65 MiB/s 497.92 MiB/s 498.18 MiB/s] |

**big:**

| Корпус    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [247.45 ms 247.83 ms 248.17 ms] thrpt:  [879.81 MiB/s 881.00 MiB/s 882.34 MiB/s] | time:   [621.91 ms 624.78 ms 627.49 ms] thrpt:  [347.96 MiB/s 349.47 MiB/s 351.08 MiB/s] | time:   [936.42 ms 937.71 ms 938.87 ms] thrpt:  [232.55 MiB/s 232.84 MiB/s 233.16 MiB/s] | time:   [896.16 ms 897.18 ms 898.18 ms] thrpt:  [243.09 MiB/s 243.36 MiB/s 243.64 MiB/s] |
| `objects` | time:   [522.66 ms 523.26 ms 523.87 ms] thrpt:  [287.02 MiB/s 287.36 MiB/s 287.69 MiB/s] | time:   [698.37 ms 699.36 ms 700.64 ms] thrpt:  [214.61 MiB/s 215.00 MiB/s 215.31 MiB/s] | time:   [680.63 ms 681.85 ms 683.01 ms] thrpt:  [220.15 MiB/s 220.52 MiB/s 220.92 MiB/s] | time:   [465.96 ms 466.57 ms 467.18 ms] thrpt:  [321.85 MiB/s 322.27 MiB/s 322.69 MiB/s] |
| `nested`  | time:   [631.63 ms 638.39 ms 644.40 ms] thrpt:  [190.09 MiB/s 191.88 MiB/s 193.93 MiB/s] | time:   [751.56 ms 755.80 ms 760.03 ms] thrpt:  [161.17 MiB/s 162.07 MiB/s 162.99 MiB/s] | time:   [711.34 ms 713.21 ms 715.77 ms] thrpt:  [171.14 MiB/s 171.75 MiB/s 172.20 MiB/s] | time:   [538.68 ms 539.50 ms 540.27 ms] thrpt:  [226.73 MiB/s 227.05 MiB/s 227.40 MiB/s] |

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_compare --features avx2`

> meon на AVX2; `simd-json` / `sonic-rs` на собственном рантайм-SIMD.

**small:**

| Корпус    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [2.0152 ms 2.0161 ms 2.0171 ms] thrpt:  [940.58 MiB/s 941.05 MiB/s 941.49 MiB/s] | time:   [4.2121 ms 4.2186 ms 4.2256 ms] thrpt:  [449.00 MiB/s 449.74 MiB/s 450.43 MiB/s] | time:   [7.7677 ms 7.7922 ms 7.8179 ms] thrpt:  [242.68 MiB/s 243.48 MiB/s 244.25 MiB/s] | time:   [2.6897 ms 2.7041 ms 2.7197 ms] thrpt:  [697.61 MiB/s 701.62 MiB/s 705.39 MiB/s] |
| `objects` | time:   [3.7105 ms 3.7142 ms 3.7182 ms] thrpt:  [373.62 MiB/s 374.02 MiB/s 374.40 MiB/s] | time:   [5.2340 ms 5.2390 ms 5.2442 ms] thrpt:  [264.90 MiB/s 265.16 MiB/s 265.42 MiB/s] | time:   [1.8550 ms 1.8619 ms 1.8694 ms] thrpt:  [743.14 MiB/s 746.12 MiB/s 748.91 MiB/s] | time:   [1.6545 ms 1.6566 ms 1.6590 ms] thrpt:  [837.36 MiB/s 838.57 MiB/s 839.63 MiB/s] |
| `nested`  | time:   [4.3796 ms 4.3852 ms 4.3908 ms] thrpt:  [257.26 MiB/s 257.59 MiB/s 257.92 MiB/s] | time:   [5.6681 ms 5.6761 ms 5.6847 ms] thrpt:  [198.71 MiB/s 199.01 MiB/s 199.29 MiB/s] | time:   [2.1097 ms 2.1134 ms 2.1176 ms] thrpt:  [533.42 MiB/s 534.49 MiB/s 535.42 MiB/s] | time:   [2.1642 ms 2.1662 ms 2.1682 ms] thrpt:  [520.97 MiB/s 521.46 MiB/s 521.93 MiB/s] |

**big:**

| Корпус    | `meon-structural` | `meon-typed` | `simd-json` | `sonic-rs` |
|-----------|-------------------|--------------|-------------|------------|
| `numbers` | time:   [208.97 ms 209.28 ms 209.85 ms] thrpt:  [1.0161 GiB/s 1.0188 GiB/s 1.0204 GiB/s] | time:   [565.28 ms 567.39 ms 569.32 ms] thrpt:  [383.51 MiB/s 384.81 MiB/s 386.25 MiB/s] | time:   [909.24 ms 912.25 ms 915.57 ms] thrpt:  [238.47 MiB/s 239.34 MiB/s 240.13 MiB/s] | time:   [910.24 ms 915.12 ms 919.94 ms] thrpt:  [237.34 MiB/s 238.59 MiB/s 239.87 MiB/s] |
| `objects` | time:   [504.00 ms 504.50 ms 505.08 ms] thrpt:  [297.70 MiB/s 298.04 MiB/s 298.34 MiB/s] | time:   [714.67 ms 719.04 ms 723.33 ms] thrpt:  [207.88 MiB/s 209.12 MiB/s 210.39 MiB/s] | time:   [671.77 ms 672.50 ms 673.24 ms] thrpt:  [223.34 MiB/s 223.59 MiB/s 223.83 MiB/s] | time:   [460.27 ms 461.38 ms 462.47 ms] thrpt:  [325.13 MiB/s 325.90 MiB/s 326.68 MiB/s] |
| `nested`  | time:   [604.64 ms 606.10 ms 607.43 ms] thrpt:  [201.66 MiB/s 202.10 MiB/s 202.59 MiB/s] | time:   [766.82 ms 768.29 ms 769.73 ms] thrpt:  [159.14 MiB/s 159.44 MiB/s 159.74 MiB/s] | time:   [708.05 ms 712.20 ms 716.15 ms] thrpt:  [171.05 MiB/s 172.00 MiB/s 173.00 MiB/s] | time:   [529.42 ms 530.75 ms 532.59 ms] thrpt:  [230.00 MiB/s 230.79 MiB/s 231.37 MiB/s] |

---

## Масштабирование от small к big

Как каждый парсер держится, когда вход вырастает за пределы кэша (сборка stable,
медианный `thrpt`, MiB/s):

| Парсер            | `numbers`     | `objects`     | `nested`      |
|-------------------|---------------|---------------|---------------|
| `meon-structural` | 815 -> 881    | 357 -> 287    | 243 -> 192    |
| `meon-typed`      | 394 -> 349    | 271 -> 215    | 201 -> 162    |
| `simd-json`       | 232 -> 233    | 708 -> 221    | 522 -> 172    |
| `sonic-rs`        | 686 -> 243    | 792 -> 322    | 498 -> 227    |

- **meon почти не деградирует с масштабом.** `meon-structural` даже выигрывает на
  `numbers` (815 -> 881) и теряет лишь ~20% на `objects`/`nested`; `meon-typed`
  следует за ним. Плоская таблица спанов остаётся преимущественно в кэше.
- **Валидирующие парсеры обваливаются на структурных корпусах на big.**
  `simd-json` теряет ~69% на `objects` и ~67% на `nested`; `sonic-rs` теряет
  ~55–65% по всему фронту — материализуя tape / владеющее `Value`, их рабочее
  множество выбивает кэш по мере роста документа. (`simd-json` держится на
  `numbers`, где его tape остаётся компактным; `sonic-rs` проседает и там.)
- **Картина переворачивается с масштабом.** На small валидирующие парсеры
  опережают `objects`/`nested` в 2–3 раза; на big это преимущество исчезает или
  инвертируется — например, на `objects` `meon-structural` обгоняет `simd-json`
  (287 против 221), а на `numbers` meon лидирует на каждом масштабе и
  увеличивает отрыв на big. Плоская таблица спанов деградирует куда меньше, чем
  материализованный tape или владеющее значение. Прогон AVX2 показывает ту же
  картину.

---

## Поэлементное извлечение meon-json (без аналога у компараторов)

`find_*` сканирует исходник по **одному** виду элемента — например, по каждой
строке — без межэлементного контекста. У `simd-json` и `sonic-rs` аналога нет:
вытащить из них только строки означает сначала материализовать весь tape /
владеющее `Value`. Числа ниже — только meon; они здесь, потому что извлечение
одного вида — часть той самой архитектурной разницы, о которой этот документ.

Каждая строка показывает счётчики `full` против `standalone`. Для JSON они
расходятся сильнее, чем для плоского формата, потому что `find_*`
**нечувствителен к вложенности**: `find_objects` совпадает только с буквальным
разделителем `{` и не отслеживает глубину, поэтому на `nested` он видит 2M / 20k
объектов верхнего уровня вместо 5M / 50k, которые разрешает полный разбор, а
`find_members` недосчитывается так же. `find_strings` точен (у содержимого строк
нет вложенности), поэтому именно он рекомендуется для одиночного прохода. Это
задокументированный компромисс — берите `find_*` только для
нечувствительного к вложенности прохода; используйте полный `parse`, когда нужна
корректная вложенность (см.
[`ARCHITECTURE.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE_RU.md#12-standalone-итераторы)).
Показано для `small` и `big`.

### stable - `cargo bench --bench meon-json_standalone`

**small:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [21.887 µs 22.547 µs 23.372 µs]
                        thrpt:  [79.276 GiB/s 82.174 GiB/s 84.655 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [23.961 µs 24.212 µs 24.447 µs]
                        thrpt:  [75.789 GiB/s 76.525 GiB/s 77.325 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [18.546 µs 18.803 µs 19.068 µs]
                        thrpt:  [97.170 GiB/s 98.537 GiB/s 99.905 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [18.938 µs 19.237 µs 19.521 µs]
                        thrpt:  [94.914 GiB/s 96.314 GiB/s 97.836 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=    20000  standalone=    20000
                        time:   [300.05 µs 300.35 µs 300.86 µs]
                        thrpt:  [4.5092 GiB/s 4.5168 GiB/s 4.5214 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [18.863 µs 19.162 µs 19.474 µs]
                        thrpt:  [69.662 GiB/s 70.797 GiB/s 71.920 GiB/s]

  find_strings   full=   120000  standalone=   120000
                        time:   [1.7571 ms 1.7591 ms 1.7616 ms]
                        thrpt:  [788.58 MiB/s 789.71 MiB/s 790.61 MiB/s]

  find_members   full=   100000  standalone=   100000
                        time:   [1.7185 ms 1.7196 ms 1.7211 ms]
                        thrpt:  [807.14 MiB/s 807.85 MiB/s 808.38 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=    50000  standalone=    20000
                        time:   [286.75 µs 287.54 µs 288.45 µs]
                        thrpt:  [3.8243 GiB/s 3.8363 GiB/s 3.8470 GiB/s]

  find_arrays    full=    10001  standalone=    10000
                        time:   [144.90 µs 145.23 µs 145.71 µs]
                        thrpt:  [7.5703 GiB/s 7.5954 GiB/s 7.6129 GiB/s]

  find_strings   full=   130000  standalone=   130000
                        time:   [1.8986 ms 1.8992 ms 1.8999 ms]
                        thrpt:  [594.55 MiB/s 594.76 MiB/s 594.94 MiB/s]

  find_members   full=   100000  standalone=    60000
                        time:   [1.0543 ms 1.0545 ms 1.0547 ms]
                        thrpt:  [1.0459 GiB/s 1.0461 GiB/s 1.0463 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [8.6214 ms 8.6317 ms 8.6424 ms]
                        thrpt:  [24.671 GiB/s 24.702 GiB/s 24.732 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [9.4820 ms 9.4955 ms 9.5104 ms]
                        thrpt:  [22.420 GiB/s 22.455 GiB/s 22.487 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [8.7343 ms 8.7507 ms 8.7761 ms]
                        thrpt:  [24.296 GiB/s 24.366 GiB/s 24.412 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [8.6625 ms 8.6893 ms 8.7301 ms]
                        thrpt:  [24.424 GiB/s 24.538 GiB/s 24.614 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=  2000000  standalone=  2000000
                        time:   [29.963 ms 30.002 ms 30.054 ms]
                        thrpt:  [4.8859 GiB/s 4.8943 GiB/s 4.9007 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [6.4211 ms 6.4309 ms 6.4452 ms]
                        thrpt:  [22.783 GiB/s 22.833 GiB/s 22.868 GiB/s]

  find_strings   full= 12000000  standalone= 12000000
                        time:   [175.45 ms 175.51 ms 175.56 ms]
                        thrpt:  [856.46 MiB/s 856.73 MiB/s 856.99 MiB/s]

  find_members   full= 10000000  standalone= 10000000
                        time:   [171.80 ms 171.94 ms 172.09 ms]
                        thrpt:  [873.73 MiB/s 874.51 MiB/s 875.22 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=  5000000  standalone=  2000000
                        time:   [29.343 ms 29.389 ms 29.428 ms]
                        thrpt:  [4.0649 GiB/s 4.0704 GiB/s 4.0768 GiB/s]

  find_arrays    full=  1000001  standalone=  1000000
                        time:   [15.350 ms 15.357 ms 15.367 ms]
                        thrpt:  [7.7845 GiB/s 7.7893 GiB/s 7.7929 GiB/s]

  find_strings   full= 13000000  standalone= 13000000
                        time:   [189.99 ms 190.07 ms 190.15 ms]
                        thrpt:  [644.19 MiB/s 644.47 MiB/s 644.74 MiB/s]

  find_members   full= 10000000  standalone=  6000000
                        time:   [107.06 ms 107.14 ms 107.22 ms]
                        thrpt:  [1.1157 GiB/s 1.1165 GiB/s 1.1173 GiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features avx2`

**small:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [19.921 µs 20.352 µs 20.695 µs]
                        thrpt:  [89.529 GiB/s 91.039 GiB/s 93.010 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [25.058 µs 25.646 µs 26.405 µs]
                        thrpt:  [70.169 GiB/s 72.245 GiB/s 73.941 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [19.767 µs 20.265 µs 20.730 µs]
                        thrpt:  [89.377 GiB/s 91.427 GiB/s 93.732 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [22.369 µs 22.722 µs 23.061 µs]
                        thrpt:  [80.344 GiB/s 81.544 GiB/s 82.830 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=    20000  standalone=    20000
                        time:   [235.90 µs 236.43 µs 237.31 µs]
                        thrpt:  [5.7167 GiB/s 5.7380 GiB/s 5.7510 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [19.039 µs 19.451 µs 19.823 µs]
                        thrpt:  [68.439 GiB/s 69.745 GiB/s 71.255 GiB/s]

  find_strings   full=   120000  standalone=   120000
                        time:   [1.2501 ms 1.2509 ms 1.2516 ms]
                        thrpt:  [1.0840 GiB/s 1.0845 GiB/s 1.0852 GiB/s]

  find_members   full=   100000  standalone=   100000
                        time:   [1.4891 ms 1.4899 ms 1.4906 ms]
                        thrpt:  [931.98 MiB/s 932.42 MiB/s 932.88 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=    50000  standalone=    20000
                        time:   [217.09 µs 217.23 µs 217.33 µs]
                        thrpt:  [5.0757 GiB/s 5.0779 GiB/s 5.0814 GiB/s]

  find_arrays    full=    10001  standalone=    10000
                        time:   [122.02 µs 122.14 µs 122.29 µs]
                        thrpt:  [9.0204 GiB/s 9.0317 GiB/s 9.0402 GiB/s]

  find_strings   full=   130000  standalone=   130000
                        time:   [1.3484 ms 1.3490 ms 1.3499 ms]
                        thrpt:  [836.80 MiB/s 837.32 MiB/s 837.75 MiB/s]

  find_members   full=   100000  standalone=    60000
                        time:   [913.13 µs 913.21 µs 913.33 µs]
                        thrpt:  [1.2078 GiB/s 1.2079 GiB/s 1.2080 GiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  find_objects   full=        0  standalone=        0
                        time:   [8.7345 ms 8.7469 ms 8.7599 ms]
                        thrpt:  [24.341 GiB/s 24.377 GiB/s 24.412 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [9.5939 ms 9.6656 ms 9.7281 ms]
                        thrpt:  [21.918 GiB/s 22.060 GiB/s 22.225 GiB/s]

  find_strings   full=        0  standalone=        0
                        time:   [8.7041 ms 8.7498 ms 8.8278 ms]
                        thrpt:  [24.153 GiB/s 24.369 GiB/s 24.497 GiB/s]

  find_members   full=        0  standalone=        0
                        time:   [8.6381 ms 8.6513 ms 8.6695 ms]
                        thrpt:  [24.594 GiB/s 24.646 GiB/s 24.684 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  find_objects   full=  2000000  standalone=  2000000
                        time:   [23.978 ms 23.997 ms 24.012 ms]
                        thrpt:  [6.1152 GiB/s 6.1190 GiB/s 6.1238 GiB/s]

  find_arrays    full=        1  standalone=        1
                        time:   [6.4343 ms 6.4465 ms 6.4665 ms]
                        thrpt:  [22.708 GiB/s 22.778 GiB/s 22.821 GiB/s]

  find_strings   full= 12000000  standalone= 12000000
                        time:   [125.18 ms 125.29 ms 125.41 ms]
                        thrpt:  [1.1709 GiB/s 1.1720 GiB/s 1.1730 GiB/s]

  find_members   full= 10000000  standalone= 10000000
                        time:   [148.79 ms 148.85 ms 148.97 ms]
                        thrpt:  [1009.4 MiB/s 1010.2 MiB/s 1010.6 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  find_objects   full=  5000000  standalone=  2000000
                        time:   [23.080 ms 23.106 ms 23.141 ms]
                        thrpt:  [5.1693 GiB/s 5.1772 GiB/s 5.1830 GiB/s]

  find_arrays    full=  1000001  standalone=  1000000
                        time:   [13.017 ms 13.026 ms 13.037 ms]
                        thrpt:  [9.1759 GiB/s 9.1833 GiB/s 9.1901 GiB/s]

  find_strings   full= 13000000  standalone= 13000000
                        time:   [134.78 ms 134.82 ms 134.86 ms]
                        thrpt:  [908.30 MiB/s 908.59 MiB/s 908.83 MiB/s]

  find_members   full= 10000000  standalone=  6000000
                        time:   [92.517 ms 92.559 ms 92.608 ms]
                        thrpt:  [1.2917 GiB/s 1.2924 GiB/s 1.2930 GiB/s]
```

</details>

---

## Контекстное извлечение meon-json (`context()` + `find_context_*`)

Для JSON непрозрачное правило — строка: `context(source)` картирует каждый
строковый регион за один потоковый проход, а варианты `find_context_*`
пропускают кандидатов-разделителей внутри них — `{` внутри строкового значения
больше не считается открытием объекта. Сами строки сохраняют только свой
контекст-свободный `find_strings` (они и *есть* источник контекста). Важна
граница применимости: контекст закрывает расхождение по непрозрачности строк,
но не нечувствительность к вложенности — скан `find_context_*` по-прежнему
совпадает с буквальными разделителями без отслеживания глубины (см.
[`ARCHITECTURE_RU.md §12`](https://github.com/vgnapuga/meon/blob/main/ARCHITECTURE_RU.md#12-standalone-итераторы)).

Три группы на корпус:

- `context` — постройка одного `ParseContext`. Карта строится один раз на
  источник и разделяется всеми `find_context_*` по нему, так что эта цена
  амортизируется между видами элементов.
- `find_context_*` — скан по уже построенной карте; счётчики `full` против
  `context-aware` показаны рядом.
- `find_context_*_cold` — постройка карты плюс скан за один вызов: цена
  одноразового запуска без переиспользования карты.

Показано для `small` и `big`.

### stable - `cargo bench --bench meon-json_standalone`

**small:**

<details>
<summary>numbers</summary>

```
  context regions (strings): 0
                        time:   [20.995 µs 21.512 µs 22.088 µs]
                        thrpt:  [83.882 GiB/s 86.128 GiB/s 88.249 GiB/s]

  find_context_objects full=        0  context-aware=        0
                        time:   [19.225 µs 20.076 µs 20.860 µs]
                        thrpt:  [88.820 GiB/s 92.288 GiB/s 96.376 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [24.026 µs 24.459 µs 24.821 µs]
                        thrpt:  [74.648 GiB/s 75.752 GiB/s 77.118 GiB/s]

  find_context_objects_cold
                        time:   [40.340 µs 40.762 µs 41.184 µs]
                        thrpt:  [44.989 GiB/s 45.455 GiB/s 45.930 GiB/s]

  find_context_arrays_cold
                        time:   [48.881 µs 49.909 µs 50.950 µs]
                        thrpt:  [36.365 GiB/s 37.124 GiB/s 37.905 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  context regions (strings): 120000
                        time:   [1.7226 ms 1.7236 ms 1.7245 ms]
                        thrpt:  [805.58 MiB/s 806.00 MiB/s 806.46 MiB/s]

  find_context_objects full=    20000  context-aware=    20000
                        time:   [402.79 µs 404.65 µs 407.52 µs]
                        thrpt:  [3.3290 GiB/s 3.3526 GiB/s 3.3681 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [77.066 µs 78.418 µs 79.960 µs]
                        thrpt:  [16.966 GiB/s 17.300 GiB/s 17.603 GiB/s]

  find_context_objects_cold
                        time:   [2.1215 ms 2.1230 ms 2.1243 ms]
                        thrpt:  [653.94 MiB/s 654.36 MiB/s 654.81 MiB/s]

  find_context_arrays_cold
                        time:   [1.7940 ms 1.7951 ms 1.7963 ms]
                        thrpt:  [773.37 MiB/s 773.89 MiB/s 774.34 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  context regions (strings): 130000
                        time:   [1.8486 ms 1.8493 ms 1.8500 ms]
                        thrpt:  [610.58 MiB/s 610.82 MiB/s 611.05 MiB/s]

  find_context_objects full=    50000  context-aware=    20000
                        time:   [361.97 µs 363.60 µs 366.35 µs]
                        thrpt:  [3.0111 GiB/s 3.0339 GiB/s 3.0475 GiB/s]

  find_context_arrays full=    10001  context-aware=    10000
                        time:   [228.93 µs 229.52 µs 229.93 µs]
                        thrpt:  [4.7976 GiB/s 4.8061 GiB/s 4.8186 GiB/s]

  find_context_objects_cold
                        time:   [2.2165 ms 2.2210 ms 2.2269 ms]
                        thrpt:  [507.24 MiB/s 508.59 MiB/s 509.62 MiB/s]

  find_context_arrays_cold
                        time:   [2.0827 ms 2.0868 ms 2.0897 ms]
                        thrpt:  [540.55 MiB/s 541.30 MiB/s 542.36 MiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  context regions (strings): 0
                        time:   [8.7870 ms 8.7936 ms 8.7991 ms]
                        thrpt:  [24.232 GiB/s 24.247 GiB/s 24.265 GiB/s]

  find_context_objects full=        0  context-aware=        0
                        time:   [8.6355 ms 8.6422 ms 8.6498 ms]
                        thrpt:  [24.650 GiB/s 24.672 GiB/s 24.691 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [9.4881 ms 9.4997 ms 9.5118 ms]
                        thrpt:  [22.416 GiB/s 22.445 GiB/s 22.472 GiB/s]

  find_context_objects_cold
                        time:   [17.307 ms 17.328 ms 17.351 ms]
                        thrpt:  [12.289 GiB/s 12.305 GiB/s 12.320 GiB/s]

  find_context_arrays_cold
                        time:   [18.189 ms 18.232 ms 18.285 ms]
                        thrpt:  [11.661 GiB/s 11.695 GiB/s 11.723 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  context regions (strings): 12000000
                        time:   [213.67 ms 214.09 ms 214.60 ms]
                        thrpt:  [700.66 MiB/s 702.35 MiB/s 703.70 MiB/s]

  find_context_objects full=  2000000  context-aware=  2000000
                        time:   [43.688 ms 43.739 ms 43.774 ms]
                        thrpt:  [3.3544 GiB/s 3.3572 GiB/s 3.3611 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [13.207 ms 13.239 ms 13.267 ms]
                        thrpt:  [11.068 GiB/s 11.092 GiB/s 11.119 GiB/s]

  find_context_objects_cold
                        time:   [255.77 ms 256.05 ms 256.40 ms]
                        thrpt:  [586.43 MiB/s 587.24 MiB/s 587.89 MiB/s]

  find_context_arrays_cold
                        time:   [226.35 ms 226.71 ms 227.12 ms]
                        thrpt:  [662.03 MiB/s 663.25 MiB/s 664.30 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  context regions (strings): 13000000
                        time:   [231.42 ms 231.69 ms 231.94 ms]
                        thrpt:  [528.13 MiB/s 528.70 MiB/s 529.31 MiB/s]

  find_context_objects full=  5000000  context-aware=  2000000
                        time:   [38.256 ms 38.349 ms 38.405 ms]
                        thrpt:  [3.1148 GiB/s 3.1193 GiB/s 3.1269 GiB/s]

  find_context_arrays full=  1000001  context-aware=  1000000
                        time:   [22.765 ms 22.884 ms 22.980 ms]
                        thrpt:  [5.2056 GiB/s 5.2273 GiB/s 5.2548 GiB/s]

  find_context_objects_cold
                        time:   [269.40 ms 269.73 ms 270.02 ms]
                        thrpt:  [453.66 MiB/s 454.14 MiB/s 454.70 MiB/s]

  find_context_arrays_cold
                        time:   [253.48 ms 253.74 ms 254.11 ms]
                        thrpt:  [482.05 MiB/s 482.76 MiB/s 483.25 MiB/s]
```

</details>

### nightly - `RUSTFLAGS="-C target-cpu=native" cargo bench --bench meon-json_standalone --features avx2`

**small:**

<details>
<summary>numbers</summary>

```
  context regions (strings): 0
                        time:   [20.624 µs 21.159 µs 21.860 µs]
                        thrpt:  [84.759 GiB/s 87.565 GiB/s 89.836 GiB/s]

  find_context_objects full=        0  context-aware=        0
                        time:   [19.940 µs 20.953 µs 22.210 µs]
                        thrpt:  [83.424 GiB/s 88.426 GiB/s 92.919 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [23.764 µs 24.126 µs 24.423 µs]
                        thrpt:  [75.863 GiB/s 76.798 GiB/s 77.968 GiB/s]

  find_context_objects_cold
                        time:   [39.562 µs 40.352 µs 41.387 µs]
                        thrpt:  [44.768 GiB/s 45.917 GiB/s 46.833 GiB/s]

  find_context_arrays_cold
                        time:   [44.927 µs 46.463 µs 47.599 µs]
                        thrpt:  [38.926 GiB/s 39.877 GiB/s 41.241 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  context regions (strings): 120000
                        time:   [1.3703 ms 1.3715 ms 1.3727 ms]
                        thrpt:  [1012.0 MiB/s 1012.9 MiB/s 1013.8 MiB/s]

  find_context_objects full=    20000  context-aware=    20000
                        time:   [298.52 µs 298.95 µs 299.28 µs]
                        thrpt:  [4.5330 GiB/s 4.5380 GiB/s 4.5446 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [49.051 µs 50.612 µs 52.413 µs]
                        thrpt:  [25.883 GiB/s 26.804 GiB/s 27.658 GiB/s]

  find_context_objects_cold
                        time:   [1.6808 ms 1.6817 ms 1.6826 ms]
                        thrpt:  [825.62 MiB/s 826.07 MiB/s 826.48 MiB/s]

  find_context_arrays_cold
                        time:   [1.4155 ms 1.4159 ms 1.4164 ms]
                        thrpt:  [980.81 MiB/s 981.14 MiB/s 981.39 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  context regions (strings): 130000
                        time:   [1.4703 ms 1.4705 ms 1.4707 ms]
                        thrpt:  [768.04 MiB/s 768.17 MiB/s 768.28 MiB/s]

  find_context_objects full=    50000  context-aware=    20000
                        time:   [270.63 µs 270.83 µs 271.03 µs]
                        thrpt:  [4.0700 GiB/s 4.0731 GiB/s 4.0761 GiB/s]

  find_context_arrays full=    10001  context-aware=    10000
                        time:   [176.55 µs 176.85 µs 177.11 µs]
                        thrpt:  [6.2285 GiB/s 6.2375 GiB/s 6.2481 GiB/s]

  find_context_objects_cold
                        time:   [1.7445 ms 1.7455 ms 1.7464 ms]
                        thrpt:  [646.81 MiB/s 647.14 MiB/s 647.50 MiB/s]

  find_context_arrays_cold
                        time:   [1.6503 ms 1.6514 ms 1.6524 ms]
                        thrpt:  [683.61 MiB/s 684.03 MiB/s 684.46 MiB/s]
```

</details>

**big:**

<details>
<summary>numbers</summary>

```
  context regions (strings): 0
                        time:   [8.8090 ms 8.8233 ms 8.8456 ms]
                        thrpt:  [24.105 GiB/s 24.166 GiB/s 24.205 GiB/s]

  find_context_objects full=        0  context-aware=        0
                        time:   [8.7020 ms 8.7245 ms 8.7511 ms]
                        thrpt:  [24.365 GiB/s 24.439 GiB/s 24.503 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [9.5281 ms 9.5431 ms 9.5600 ms]
                        thrpt:  [22.303 GiB/s 22.343 GiB/s 22.378 GiB/s]

  find_context_objects_cold
                        time:   [17.470 ms 17.544 ms 17.599 ms]
                        thrpt:  [12.116 GiB/s 12.154 GiB/s 12.205 GiB/s]

  find_context_arrays_cold
                        time:   [18.347 ms 18.400 ms 18.488 ms]
                        thrpt:  [11.533 GiB/s 11.588 GiB/s 11.622 GiB/s]
```

</details>

<details>
<summary>objects</summary>

```
  context regions (strings): 12000000
                        time:   [177.93 ms 178.13 ms 178.33 ms]
                        thrpt:  [843.15 MiB/s 844.10 MiB/s 845.07 MiB/s]

  find_context_objects full=  2000000  context-aware=  2000000
                        time:   [32.940 ms 33.036 ms 33.093 ms]
                        thrpt:  [4.4371 GiB/s 4.4448 GiB/s 4.4577 GiB/s]

  find_context_arrays full=        1  context-aware=        1
                        time:   [11.472 ms 11.479 ms 11.487 ms]
                        thrpt:  [12.783 GiB/s 12.792 GiB/s 12.799 GiB/s]

  find_context_objects_cold
                        time:   [211.04 ms 211.38 ms 211.72 ms]
                        thrpt:  [710.18 MiB/s 711.35 MiB/s 712.48 MiB/s]

  find_context_arrays_cold
                        time:   [190.79 ms 190.90 ms 191.02 ms]
                        thrpt:  [787.16 MiB/s 787.64 MiB/s 788.12 MiB/s]
```

</details>

<details>
<summary>nested</summary>

```
  context regions (strings): 13000000
                        time:   [192.18 ms 192.33 ms 192.48 ms]
                        thrpt:  [636.40 MiB/s 636.88 MiB/s 637.38 MiB/s]

  find_context_objects full=  5000000  context-aware=  2000000
                        time:   [29.057 ms 29.077 ms 29.110 ms]
                        thrpt:  [4.1093 GiB/s 4.1140 GiB/s 4.1169 GiB/s]

  find_context_arrays full=  1000001  context-aware=  1000000
                        time:   [18.240 ms 18.259 ms 18.277 ms]
                        thrpt:  [6.5451 GiB/s 6.5515 GiB/s 6.5583 GiB/s]

  find_context_objects_cold
                        time:   [222.91 ms 223.11 ms 223.32 ms]
                        thrpt:  [548.51 MiB/s 549.03 MiB/s 549.51 MiB/s]

  find_context_arrays_cold
                        time:   [213.06 ms 213.28 ms 213.49 ms]
                        thrpt:  [573.77 MiB/s 574.32 MiB/s 574.92 MiB/s]
```

</details>

---

## Как читать числа

- Большее число у meon отражает его задачу — векторы спанов, без валидации, без
  парсинга чисел, без разэкранирования. Потребитель, которому нужны
  типизированные значения или декодированные строки, делает эту работу поверх
  спанов meon.
- Сравнивайте ячейку только с тем же корпусом в том же блоке сборки.
- `simd-json` и `sonic-rs` сразу выдают готовые к использованию значения; meon
  выдаёт спаны, из которых вы проецируете. Разрыв — это стоимость той
  материализации, которая вашему сценарию может быть нужна, а может и нет.
- Корпус `numbers` показывает наибольший разрыв по построению; `objects` и
  `nested` ближе к смешанной реальной нагрузке. Масштаб важнее заголовка на
  малом входе — см. [Масштабирование от small к big](#масштабирование-от-small-к-big).
