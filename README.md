<p align="center">
  <h1 align="center">DriftGlide</h1>
  <p align="center">
    <b>Gesture navigation bar & Circle to Search for Linux Wayland</b>
  </p>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-Linux%20Wayland-blue" alt="Platform">
  <img src="https://img.shields.io/badge/language-Rust-orange" alt="Language">
  <img src="https://img.shields.io/badge/rendering-tiny--skia-green" alt="Renderer">
  <img src="https://img.shields.io/badge/AI-Gemini%20Vision-purple" alt="AI">
  <img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License">
</p>

---

**DriftGlide** — демон навигации для Wayland-композиторов, реализующий жестовую навигационную полоску (Navigation Bar), переключатель окон (Task Switcher) и функцию **Circle to Search** с интеграцией Google Gemini Vision API для анализа выделенных областей экрана.

Проект является компаньоном оконного менеджера [driftwm](https://github.com/wwmaxik/driftwm) и взаимодействует с ним через IPC-сокет.

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
- Вырезание фрагмента и отправка в **Google Gemini Vision API**
- Плавающее окно чата с Gemini:
  - Полноценный Markdown-рендеринг ответов (заголовки, код, списки, цитаты, разделители)
  - Диалоговый режим с историей сообщений
  - Миниатюра вырезанного фрагмента экрана
  - Drag-перемещение окна за шапку
  - Сворачивание в компактный плавающий кружок
  - Прокрутка колесом мыши и скроллбаром
  - Выбор модели Gemini через интерактивные чипы
  - Настройка API-ключа через UI

### ⌨️ Полная поддержка клавиатуры
- `Ctrl+A` / `Ctrl+Ф` — выделить всё (текст ввода или ответ)
- `Ctrl+C` / `Ctrl+С` — копировать выделенный текст
- `Ctrl+X` / `Ctrl+Ч` — вырезать
- `Ctrl+V` / `Ctrl+М` — вставить из буфера обмена
- `Shift+←/→/Home/End` — выделение с клавиатуры
- Двойной клик — выделение слова, тройной — всего текста
- Drag-выделение текста мышью в поле ввода и в области ответа с автоскроллом

## 📋 Системные требования

- **Linux** с **Wayland**-композитором
- Поддержка протокола `wlr-layer-shell-v1` (Sway, Hyprland, wlroots-based, KDE Plasma 6+)
- **Rust** ≥ 1.70 (для сборки)
- `wl-copy` / `wl-paste` — для работы с буфером обмена
- `spectacle` или `grim` — для захвата экрана (если `zwlr_screencopy_v1` недоступен)

## 🚀 Сборка и запуск

```bash
# Клонирование
git clone https://github.com/wwmaxik/driftglide.git
cd driftglide

# Сборка
cargo build --release

# Запуск
./target/release/driftglide
```

### Переменные окружения

| Переменная | Описание |
|---|---|
| `GEMINI_API_KEY` | API-ключ Google Gemini (или сохраните в `~/.config/driftglide/gemini_key`) |
| `DRIFTWM_SOCKET` | Путь к IPC-сокету driftwm (определяется автоматически) |
| `RUST_LOG` | Уровень логирования (`driftglide=info`, `driftglide=debug`) |

## ⚙️ Конфигурация

API-ключ Gemini можно задать тремя способами:
1. Переменная окружения `GEMINI_API_KEY`
2. Файл `~/.config/driftglide/gemini_key`
3. Через UI — нажмите ⚙ в окне Gemini и введите ключ

Модель Gemini выбирается через интерактивные чипы в настройках окна:
- **Gemini 3.1 Flash Lite** (по умолчанию)
- **Gemini 3.5 Flash Lite**

## 🏗️ Архитектура

```
src/
├── main.rs              # Точка входа, event loop (calloop)
├── config.rs            # Конфигурация и тема оформления
├── gestures.rs          # Распознавание жестов (свайпы, долгое нажатие, флики)
├── search.rs            # Circle to Search (лассо, скриншот, кроп)
├── ipc.rs               # IPC-клиент для связи с driftwm
├── gemini/
│   ├── mod.rs
│   ├── client.rs        # HTTP-клиент Gemini Vision API
│   └── window.rs        # Состояние и логика окна Gemini (ввод, выделение, чат)
├── render/
│   ├── mod.rs
│   ├── text.rs          # Рендеринг текста через fontdue
│   ├── markdown.rs      # Layout и рендеринг Markdown
│   ├── gemini.rs        # Отрисовка окна Gemini (UI, кнопки, скроллбар)
│   ├── pill.rs          # Отрисовка навигационной полоски
│   ├── lasso.rs         # Отрисовка лассо и скриншота
│   ├── launcher.rs      # Отрисовка Task Switcher
│   └── icon.rs          # Векторные иконки (шестерёнка, искра, лупа и др.)
└── wayland/
    ├── mod.rs           # AppState, Wayland Dispatch (pointer, touch, keyboard)
    ├── layer_surface.rs # Создание Layer Shell поверхностей
    ├── shm.rs           # Тройная буферизация SHM для плавного рендеринга
    ├── screencopy.rs    # Захват экрана через zwlr_screencopy_v1
    └── touch.rs         # Вспомогательные утилиты тач-ввода
```

### Стек технологий

| Компонент | Технология |
|---|---|
| Дисплейный протокол | Wayland (`wayland-client`, `wlr-layer-shell-v1`, `wlr-screencopy-v1`) |
| Event loop | `calloop` + `calloop-wayland-source` |
| 2D рендеринг | `tiny-skia` (CPU, без GPU) |
| Шрифты | `fontdue` (растеризация) + `resvg` (SVG) |
| Клавиатура | `xkbcommon` (XKB раскладки, модификаторы) |
| AI | Google Gemini Vision API (HTTP + JSON) |
| IPC | Unix Domain Socket (JSON-протокол с driftwm) |
| Буфер обмена | `wl-copy` / `wl-paste` (CLI) |

## 🎨 Дизайн

- Премиальная тёмная тема с глубокими графитовыми оттенками
- Сапфировый акцентный цвет (`#4086F4`)
- Скруглённые элементы с деликатными тенями
- Плавные физические анимации открытия/закрытия/сворачивания
- Минималистичный полупрозрачный скроллбар
- Мягкая контрастная подсветка выделения текста

## 📄 Лицензия

MIT License © 2026

---

<p align="center">
  <i>Сделано с ❤️ для Linux Wayland</i>
</p>
