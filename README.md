<div align="center">

# HoverTranslate

### Translate anything. Just hover.

A lightweight Windows desktop translator that detects the text beneath your cursor and displays the translation in a clean, unobtrusive popup.

[![Latest Release](https://img.shields.io/github/v/release/didohm/hovertranslate?style=flat-square&logo=github&label=latest%20release)](https://github.com/didohm/hovertranslate/releases/latest)
[![License](https://img.shields.io/github/license/didohm/hovertranslate?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-0078D4?style=flat-square&logo=windows)](https://www.microsoft.com/windows)

[**Download for Windows**](https://github.com/didohm/hovertranslate/releases/latest) · [Report a bug](https://github.com/didohm/hovertranslate/issues) · [Source code](https://github.com/didohm/hovertranslate)

</div>

---

## What is HoverTranslate?

HoverTranslate makes reading foreign-language content feel effortless.

Move your mouse over text, pause for a moment, and its translation appears beside the pointer. No need to copy text, switch windows, or interrupt what you are doing.

It runs quietly in the background and works with text exposed by Windows accessibility APIs.

> **No OCR:** HoverTranslate reads accessible text provided by Windows UI Automation. Text that is only present inside an image or otherwise unavailable to accessibility APIs cannot be detected.

## Download

<div align="center">

### [Download the latest version](https://github.com/didohm/hovertranslate/releases/latest)

**Windows 10 or newer · 64-bit · Portable**

</div>

1. Open the [Releases page](https://github.com/didohm/hovertranslate/releases/latest).
2. Download `hovertranslate-v0.1.0-windows-x64.zip`.
3. Extract the ZIP archive.
4. Run `hovertranslate.exe`.

There is no installer. Extract it and run it.

## Features

- **Translate on hover** — detects the text beneath your pointer after a configurable pause.
- **A popup beside your cursor** — displays the translation in a compact, always-on-top card.
- **Automatic dismissal** — the card disappears when you move away from the text.
- **Language controls** — choose the source language, target language and translation service.
- **Adjustable hover delay** — set the delay from 0.2 to 3 seconds.
- **Remembers your preferences** — settings are saved between sessions.
- **Translation cache** — recently translated text is kept in memory to avoid unnecessary repeat requests.
- **Lightweight and portable** — no installation is required.

## Designed for real-world text

HoverTranslate is built to stay out of your way. Its control panel provides the settings you need, while the translation card appears only when you need it.

- A compact control panel with an enable/disable switch.
- A configurable hover delay and language selection.
- A panel that adjusts to its content and stays within the screen.
- A translation preview that wraps long text and can scroll.
- A popup that keeps long translations readable instead of clipping them.

## Arabic, Chinese and multilingual text

A translation is only useful when it is readable. HoverTranslate includes support for scripts that need more than a basic Latin font.

- **Arabic and Hebrew:** right-to-left paragraphs are aligned correctly, with joined letters and natural text order.
- **Mixed-direction text:** Arabic sentences containing English words and numbers keep their correct visual order.
- **Chinese, Japanese and Korean:** system fonts are loaded when these scripts are needed, without bundling large font files unnecessarily.
- **Long translations:** requests are split into smaller pieces when required by the free translation service, then joined back together.
- **Language codes:** provider-specific language codes are handled internally.

The app uses Windows fonts and egui's text shaping support. It does not manually reverse Arabic text.

## Screenshots

<!-- Add real application screenshots here when available.
     Suggested layout:
     - Main control panel
     - Translation popup beside the cursor
     - Arabic / RTL translation
     - Long translation wrapping
-->

## How it works

1. HoverTranslate watches the mouse pointer in the background.
2. When the pointer stays still for the configured delay, the app asks Windows UI Automation for the text beneath it.
3. The detected text is sent to the selected translation provider when a translation is needed.
4. The translated result appears in a popup beside the pointer.
5. Moving away from the text hides the popup.

Only text exposed by the accessibility interface can be read. HoverTranslate does not perform OCR or take screenshots to extract text.

## Privacy

HoverTranslate is designed to send only what is needed for translation.

- Only the text under the pointer is sent when a translation is actually requested.
- Recent translations are cached in memory for a short time.
- The translation cache is not written to disk and disappears when the app closes.
- Settings are stored locally in `%APPDATA%\HoverTranslate\settings.json`.
- The settings file contains preferences such as the enabled state, delay, languages and provider address — not hovered text or translation history.
- The public release is intended to use the backend relay so provider credentials remain on the server rather than inside the desktop executable.

As with any online translation service, the text you choose to translate is sent to that service (or the configured relay) for processing.

## Running from source

### Requirements

- Rust toolchain
- Windows development environment

### Build

```powershell
cargo build --profile official
```

Run the application:

```powershell
target\official\hovertranslate.exe
```

The `official` profile inherits the development settings and is the build profile verified on Windows.

### Useful command-line options

| Option | Description |
| --- | --- |
| `--screenshot <file.png>` | Draws the window, saves a screenshot and exits. |
| `--rtl-demo` | Displays a long mixed Arabic paragraph for checking RTL wrapping. Use with `--screenshot`. |
| `--help` | Prints the available options. |

## Project structure

```text
src/
├── main.rs          Application startup, egui window and popup
├── app.rs           Application state, pointer events and translation flow
├── ui.rs            Control panel and translation card
├── theme.rs         Colours, sizing and custom widgets
├── fonts.rs         Windows fonts and multilingual text support
├── languages.rs     Language definitions and provider language codes
├── hover.rs         Pointer watcher and text detection flow
├── uia.rs           Windows UI Automation text access
├── settings.rs      User preferences and persistence
└── translate/
    ├── mod.rs       Translation provider interface and shared helpers
    ├── mymemory.rs  MyMemory provider
    ├── backend.rs   Backend relay provider
    ├── libretranslate.rs
    ├── cache.rs     In-memory translation cache
    └── ratelimit.rs Request rate limiter
```

### Adding a translation provider

Translation services are integrated through the `TranslationProvider` trait in `src/translate/`.

Implement the trait and register the provider in `build_provider()` in `src/app.rs`. The UI, pointer watcher and translation card use the shared interface, so they do not need provider-specific changes.

```rust
pub trait TranslationProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> String;
    fn translate(
        &self,
        request: &TranslationRequest,
    ) -> Result<TranslationOutcome, TranslateError>;
}
```

Provider-specific source and target language codes are handled by `languages::provider_source()` and `languages::provider_target()`.

## Backend relay (optional)

The repository includes a small reference relay in `backend/server.py`. It receives translation requests from the desktop app and forwards them to the configured provider.

```powershell
cd backend
$env:MYMEMORY_EMAIL = "you@example.com"
python server.py
```

The app sends a `POST /translate` request:

```json
{
  "text": "hello",
  "source": "auto",
  "target": "es"
}
```

The relay responds with:

```json
{
  "translation": "hola",
  "detected_source": "en"
}
```

The same contract can be used with another translation service, including a LibreTranslate instance or a self-hosted provider.

## Testing

The `shots/` directory contains helper scripts for manually checking the hover-to-translation flow on Windows.

```powershell
# Open a window containing sample text
powershell -File shots\form.ps1

# Move the pointer to the text and capture the screen
powershell -File shots\grab3.ps1 -X 640 -Y 320
```

The application also prints detected text and provider responses to standard output to help with debugging.

## Contributing

Issues, suggestions and improvements are welcome.

- [Report a bug](https://github.com/didohm/hovertranslate/issues)
- [Browse the source code](https://github.com/didohm/hovertranslate)

When reporting a problem, include your Windows version, the steps to reproduce it and, when possible, the relevant console output.

## License

HoverTranslate is released under the [MIT License](LICENSE).
