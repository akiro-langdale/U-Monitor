# U-Monitor

Нативный монитор на Rust и egui/eframe. Текущий GUI показывает статистику RAM
и график последних 60 полученных снимков.

Лицензия: MIT OR Apache-2.0. Документация: [architecture](docs/ARCHITECTURE.md),
[security](docs/SECURITY.md), [release](docs/RELEASE.md), [changelog](CHANGELOG.md).

## Окружение разработки

Требования: Rust `1.90.0` (см. `rust-toolchain.toml`), Git, доступ к
[репозиторию](https://github.com/akiro-langdale/U-Monitor.git).

CI (`.github/workflows/ci.yml`) гоняет `fmt` / `clippy` / `test` на
Ubuntu, Windows и macOS. Ограничения backend по OS/arch видны на странице
«Обзор» и в `NativePlatformAdapter::check_availability`:

| Возможность | Windows | Linux | macOS |
| --- | --- | --- | --- |
| SystemIdentity / CPU discovery / RAM / Storage | да | да | да\* |
| Virtual memory map + HEX | да | да | нет |
| Cache topology | да | да | нет |
| Current-thread registers | x86_64 | нет | нет |
| Other-thread registers (consent) | x86_64 | да (ptrace x86_64/aarch64) | нет |
| Simulation (L1/L2/RAM) | да (SIMULATED) | да | да |

\* через `sysinfo`, где host поддерживается. Живая нагрузка CPU идёт через
`NativeTelemetryAdapter`, а не через `PlatformFeature::CpuMetrics`.
Чужой поток на странице «Регистры» требует явного согласия; без прав —
`PERMISSION_REQUIRED`, без SIMULATED.

```powershell
rustup show
cargo --version
```

Форматирование и линтер: `rustfmt.toml`, `clippy.toml`. Проверка перед коммитом:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

На странице CPU показан снимок характеристик при запуске: все обнаруженные
модели, физические ядра и логические процессоры. Каждое поле сохраняет свой
статус доступности, источник и время UTC. Отсутствующие характеристики не
заменяются нулями. Живая нагрузка CPU и частоты логических процессоров берутся из того же
фонового `TelemetryWorker`, что и RAM. На Windows частоты читаются через
`CallNtPowerInformation`; нулевые ответы backend помечаются `UNAVAILABLE`.

Фоновая телеметрия уже собирает RAM и загрузку CPU в одном `TelemetrySnapshot`.
`NativePlatformAdapter` получает статические характеристики, а отдельный
`NativeTelemetryAdapter` сохраняет счётчики CPU между вызовами и передаётся
в `TelemetryWorker`. На Windows нагрузка CPU читается через `GetSystemTimes` и
`NtQuerySystemInformation` (не PDH/`sysinfo`), потому что PDH `% Idle Time`
на части систем всегда остаётся 0. Первый CPU-снимок имеет статус
`UNAVAILABLE` (прогрев). Повторный запрос раньше
`MINIMUM_CPU_SAMPLING_INTERVAL` тоже возвращает `UNAVAILABLE`, не изменяя
базовую точку и не ожидая внутри адаптера.

В `snapshot.cpu` находятся общая загрузка и загрузка каждого логического
процессора в процентах 0–100, источник `MEASURED`, время UTC и наблюдаемый
интервал между завершёнными обновлениями счётчиков. Это интервал измерения,
а не заданная пауза фонового потока. CPU и RAM считываются последовательно,
поэтому снимок не является атомарным. Ошибки CPU не заменяют успешные данные RAM.
История хранит и успешные, и неуспешные попытки, включая прогрев.

Индексы и имена логических процессоров относятся к текущему сеансу сборщика;
горячее изменение топологии CPU пока не отслеживается. `sysinfo` не раскрывает
все ошибки обновления системных счётчиков: некоторые сбои могут выглядеть как
нулевые или прежние значения backend. Адаптер проверяет диапазон и конечность
процентов, но не может гарантированно обнаружить такие скрытые сбои.

Проверка реального CPU и RAM в фоновом потоке (Windows/Linux/macOS):

```powershell
cargo test -p u-monitor-telemetry --test native_cpu -- --ignored --nocapture
```

Страница «Накопители» показывает полученный при запуске список смонтированных
томов: имена, точки монтирования, файловые системы, тип SSD/HDD (либо UNKNOWN),
общий, доступный и занятый объём в байтах и GiB. Таблица прокручивается по
горизонтали. У полей сохраняются статусы, причины ошибок, источник и время UTC.
Тома не равнозначны физическим устройствам: их ёмкости могут пересекаться.
Занятое пространство вычисляется как общий объём минус доступный и может
включать резервы и квоты. Автоматическое обновление томов пока не реализовано.

Страница «Память» показывает карту виртуальных регионов **текущего** процесса
U-Monitor (`VirtualQuery` на Windows, `/proc/self/maps` + `/proc/self/mem` на
Linux): база, размер, состояние, доступ и тип.
По кнопке HEX читается не более 512 байт только из COMMITTED+readable
областей. Можно запомнить снимок A и сравнить с повторным чтением B:
отличия байт помечены `CALCULATED`. Это не физическая RAM и не память
чужих процессов.

Страница «Регистры» показывает MEASURED-снимок GPR текущего потока UI
(Windows x86-64: `RtlCaptureContext`): HEX/DEC/BIN и Δ к предыдущему снимку
(`CALCULATED`). Другие потоки и ISA пока не реализованы.

Страница «Кэш» показывает MEASURED-топологию иерархии CPU-кэшей (уровень,
тип, размер, размер линии, ассоциативность, shared logical CPUs). На Windows
используется `GetLogicalProcessorInformationEx(RelationCache)`, на Linux —
`/sys/devices/system/cpu/*/cache/index*`. Содержимое кэш-линий недоступно и
не реконструируется.

Страница «Симуляция» запускает учебную модель иерархии L1→L2→RAM
(set-associative LRU) из `u-monitor-simulation`. Hit/miss, eviction и latency
всегда помечены `SIMULATED` и не смешиваются с MEASURED-данными платформы.

```powershell
cargo run -p u-monitor-ui
```

## C ABI (`u-monitor-ffi`)

Публичный C ABI собирается как `cdylib`/`staticlib` с именем `u_monitor`.
Заголовок: `crates/u-monitor-ffi/include/u_monitor.h`.

```powershell
cargo build -p u-monitor-ffi
python examples/python/u_monitor.py
```

Пример на C: `examples/c/main.c`. Владение: caller-owned структуры не
освобождаются; opaque `UmHierarchy*` создаётся через `um_hierarchy_create` и
уничтожается через `um_hierarchy_destroy`. Симуляция всегда возвращает
`UM_SOURCE_SIMULATED`.

На страницах CPU и RAM можно выбрать общий интервал от 250 до 5000 мс —
оба раздела читают один `TelemetryWorker`. График RAM показывает долю
используемой памяти (`CALCULATED`: используется / общий объём). График CPU
показывает общую нагрузку (`MEASURED`) и таблицу логических процессоров.
Наведите указатель на график, чтобы увидеть значение и время снимка в UTC.
Горизонтальная шкала отсчитывается от последнего полученного снимка по
монотонному времени: изменение интервала меняет расстояние между точками.

История GUI ограничена 60 уведомлениями на каждый график, включая
неуспешные измерения. Это не фиксированное временное окно и не полная
история фонового сборщика: при переполнении канала часть уведомлений может
быть пропущена. Ошибки, недоступные значения и пропуски номеров снимков
разрывают линию, а не заменяются нулями. При остановке сборщика остаётся
последняя история.

Проверки и релиз:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace --release
```

Тег `v*` запускает `.github/workflows/release.yml` (черновик GitHub Release с
бинарниками GUI и shared library). Детали — в `docs/RELEASE.md`.
