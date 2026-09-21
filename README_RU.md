<p align="center">
  <h1 align="center">DriftGlide</h1>
  <p align="center">
    <b>Жестовая панель навигации и Circle to Search для Linux Wayland</b>
  </p>
</p>

<p align="center">
  <a href="README.md"><b>English</b></a> | <a href="README_RU.md"><b>Русский</b></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/статус-Beta-yellow" alt="Статус: Beta">
  <img src="https://img.shields.io/badge/platform-Linux%20Wayland-blue" alt="Платформа">
  <img src="https://img.shields.io/badge/language-Rust-orange" alt="Язык">
  <img src="https://img.shields.io/badge/rendering-tiny--skia-green" alt="Рендерер">
  <img src="https://img.shields.io/badge/AI-Gemini%20Vision-purple" alt="AI">
  <img src="https://img.shields.io/badge/license-GPL--3.0-blue" alt="Лицензия">
</p>

---

> [!NOTE]
> **Статус проекта: Beta**  
> DriftGlide находится на стадии активной **Beta**-разработки. Проект разрабатывается в первую очередь для **ноутбуков с сенсорным экраном (тачскрином) и устройств-трансформеров (2-в-1)**, сохраняя при этом полную поддержку мыши и тачпада.

**DriftGlide** — демон навигации для Wayland-композиторов, реализующий жестовую полоску навигации (Navigation Bar), переключатель окон (Task Switcher) и функцию **Circle to Search** с интеграцией Google Gemini Vision API для анализа выделенных областей экрана.

Проект является компаньоном оконного менеджера [driftwm](https://github.com/malbiruk/driftwm) и взаимодействует с ним через IPC-сокет.

## ✨ Возможности

### 🏠 Gesture Navigation Bar
- Нижняя интерактивная навигационная полоска с плавной анимацией
- **Свайп влево/вправо** — быстрое переключение между окнами (`focus_prev` / `focus_next`)
- **Свайп вверх** — открытие переключателя задач (Task Switcher)
- **Долгое нажатие** — активация Circle to Search
- Поддержка сенсорных экранов (тач) и мыши/тачпада
- Плавная пружинная физика возврата полоски с инерцией

### 🔲 Task Switcher
- Полноэкранный оверлей со списком открытых окон
- Получение списка окон через IPC от driftwm
- Клик по окну — фокусировка и закрытие свитчера
- Автоматическое скрытие по таймауту неактивности

### 🔍 Circle to Search + Gemini Vision
- Захват скриншота экрана (`zwlr_screencopy_v1` / `spectacle` / `grim`)
- Обводка области экрана произвольным лассо (пальцем или мышью)
- Вырезание фрагмента и автоматическое копирование в буфер обмена (`wl-copy`)
- Плавающее окно чата с Gemini:
  - Полноценный Markdown-рендеринг ответов (заголовки, код, блоки кода, списки, цитаты, разделители)
  - Диалоговый режим с историей сообщений
  - Миниатюра вырезанного фрагмента экрана
  - Drag-перемещение окна за шапку
  - Сворачивание в компактный плавающий кружок
  - Прокрутка колесом мыши и скроллбаром с автоскроллом при выделении
  - Выбор модели Gemini через интерактивные чипы (Gemini 3.1 Flash Lite / Gemini 3.5 Flash Lite)
  - Настройка API-ключа через UI

### ⌨️ Полная поддержка клавиатуры и выделения
- `Ctrl+A` / `Ctrl+Ф` — выделить всё (текст ввода или ответ)
- `Ctrl+C` / `Ctrl+С` — копировать выделенный текст (или весь ответ)
- `Ctrl+X` / `Ctrl+Ч` — вырезать
- `Ctrl+V` / `Ctrl+М` — вставить из буфера обмена
- `Shift+←/→/Home/End` — выделение с клавиатуры
- Двойной клик — выделение слова, тройной — всего текста
- Drag-выделение текста мышью в поле ввода и в области ответа с автоскроллом

## 📋 Системные требования

- **Linux** с **Wayland**-композитором
- Поддержка протокола `wlr-layer-shell-v1` (Sway, Hyprland, wlroots-based, KDE Plasma 6+)
- **Rust** ≥ 1.70 & `cargo` (для сборки)
- `wl-copy` / `wl-paste` — для работы с буфером обмена
- `spectacle` или `grim` — для захвата экрана (если `zwlr_screencopy_v1` недоступен)

## 🚀 Сборка и установка

### С использованием Makefile (рекомендуется)

```bash
# Клонирование репозитория
git clone https://github.com/wwmaxik/driftglide.git
cd driftglide

# Сборка release бинарника
make build

# Установка в систему (/usr/local/bin)
sudo make install
```

Установка с произвольным префиксом:
```bash
make PREFIX=/usr install
```

### Ручная сборка через Cargo

```bash
cargo build --release
sudo cp target/release/driftglide /usr/local/bin/
```

## 🔄 Автозагрузка в driftwm

Чтобы DriftGlide автоматически запускался вместе с **driftwm**, добавьте его в список `autostart` в вашем конфигурационном файле `~/.config/driftwm/config.toml`:

```toml
autostart = ["driftglide"]
```

Или вместе с другими сервисами:

```toml
autostart = ["waybar", "driftglide"]
```

## ⚙️ Конфигурация и переменные окружения

| Переменная | Описание |
|---|---|
| `GEMINI_API_KEY` | API-ключ Google Gemini (или сохраните в `~/.config/driftglide/gemini_key`) |
| `DRIFTWM_SOCKET` | Путь к IPC-сокету driftwm (определяется автоматически) |
| `RUST_LOG` | Уровень логирования (`driftglide=info`, `driftglide=debug`) |

### Настройка API-ключа Gemini
API-ключ Gemini можно задать тремя способами:
1. Переменная окружения `GEMINI_API_KEY`
2. Файл `~/.config/driftglide/gemini_key`
3. Через UI — нажмите ⚙ в окне Gemini и введите ключ

## 🤝 Участие в разработке (Contributing)

Мы приветствуем вклад в развитие проекта! Ознакомьтесь с [CONTRIBUTING_RU.md](CONTRIBUTING_RU.md) (или [CONTRIBUTING.md](CONTRIBUTING.md)) для информации о процессе разработки, кодстайле и оформлении Pull Request.

Пожалуйста, соблюдайте [Кодекс поведения](CODE_OF_CONDUCT_RU.md).

## 📄 Лицензия

Проект распространяется под свободной лицензией **GNU General Public License v3.0** (GPL-3.0) — полный текст доступен в файле [LICENSE](LICENSE).

---

<p align="center">
  <i>Сделано с ❤️ для DriftWM</i>
</p>
