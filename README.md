# HoverTranslate

A small Windows desktop app: rest the mouse pointer on any text, wait a
moment, and the translation appears in a card next to the pointer.

## Download

Grab the latest Windows build from
[Releases](https://github.com/didohm/hovertranslate/releases/latest) — download
`hovertranslate-v0.1.0-windows-x64.zip`, unzip it and run `hovertranslate.exe`.
It is a portable 64-bit Windows app (Windows 10 or newer); there is nothing to
install. To build it yourself instead, see [Running it](#running-it).

It is written in Rust with [egui](https://github.com/emilk/egui) (through
`eframe`) and reads text with the Windows accessibility APIs (UI Automation).
There is **no OCR**: only text that Windows itself exposes as accessible text
can be translated.

## What it does

- Watches the pointer in the background and picks up the text under it once the
  pointer has been still for the configured delay (0.2 - 3 seconds).
- Sends that text to an online translation service and shows the answer in a
  small always-on-top card next to the pointer.
- Hides the card as soon as the pointer moves away from the text.
- Keeps a control panel with a switch, the hover delay, the target language,
  the source language and the service to use. The panel is exactly as tall as
  its content and never taller than the screen; it can shrink to a small bar
  so the app can stay in the background.
- Remembers the settings between sessions.

## The look

The app is a small instrument that lives on top of other programs, so the
panel is quiet: a dark graphite ground, one accent colour, thin rules and a
lot of air. The only loud thing is the card next to the pointer.

- The mark of the app is a lens with a light in it, drawn by the app itself.
- The type is Segoe UI Variable when the machine has it, so the headings
  really are a weight heavier than the rest.
- The panel is measured after it is drawn, so it is never cut off and never
  has empty space at the bottom. It grows and shrinks with its content and
  stays inside the screen.
- A roomy preview card shows the last translation (or a short sample). Long
  text wraps and scrolls inside the card, so no word, button or label is ever
  clipped.
- The card beside the pointer wraps whole sentences and scrolls when they are
  very long, instead of cutting the text short.

## Arabic, Chinese and other scripts

Translations are shown in the language they are written in, which means the
app has to know more than Latin:

- **The fonts.** egui only ships a Latin font, so a translated Arabic or
  Chinese sentence would arrive as a row of empty boxes. The app reads the
  Windows fonts when it starts: Segoe UI Variable and Segoe UI for Latin,
  Arabic and Hebrew. The big CJK fonts (Microsoft YaHei, Microsoft JhengHei,
  Yu Gothic, Malgun Gothic) are only read when a character of that script
  actually shows up, so the app does not carry fifty megabytes of font data
  around for nothing.
- **The writing direction.** Arabic and Hebrew are lined up on the right, and
  the letters are joined the way they are written, not one after the other.
  egui shapes text with HarfBuzz, which joins Arabic letters and puts the runs
  of a right-to-left sentence into visual order by itself. The app only detects
  the direction of each paragraph (from its first strong letter, like
  `unicode-bidi: plaintext`) and right-aligns RTL blocks, like `direction: rtl`
  and `text-align: right`. The text is never reversed or split up by hand, so
  a long Arabic paragraph wraps in the natural order: its first line is the
  beginning of the sentence, and mixed text such as "مرحبا Hello world 123"
  keeps the Latin words and numbers in their normal order.
- **The language codes.** Every service names the languages a little
  differently: LibreTranslate knows Chinese as `zh` where MyMemory wants
  `zh-CN` or `zh-TW`, and MyMemory calls the automatic source `autodetect`.
  The app keeps its own codes and turns them into the right ones on the way
  out (see `src/languages.rs`).
- **The size of the request.** The free service counts the request in bytes,
  and an Arabic letter is two bytes and a Chinese letter three. A long
  sentence in those languages is therefore cut into smaller pieces at a full
  stop, a space or between two Chinese characters, and the answers are put
  back together afterwards.
- **The answers of the service.** The free service sometimes answers with a
  notice about the word instead of a translation, for example
  `[ترجمة المصطلح: Hello]`. That notice is taken off, and when there is no
  translation at all the card says so instead of showing the notice.

## Running it

```sh
cargo build --profile official
target\official\hovertranslate.exe
```

The `official` profile inherits the dev settings; it is the build that has
been verified on Windows.

Useful flags:

| Flag | What it does |
| --- | --- |
| `--screenshot <file.png>` | Draws for a moment, saves a picture of the window and exits |
| `--rtl-demo` | Shows a long mixed Arabic paragraph in the preview, for checking RTL wrapping (pair it with `--screenshot`) |
| `--help` | Prints the same list |

## How it is put together

```
src/
  main.rs        start-up, the egui window, the popup window, the screenshot mode
  app.rs         the state of the app: pointer events, translation requests, cache
  ui.rs          drawing of the control panel and of the card next to the pointer
  theme.rs       the colours, the sizes and the widgets egui does not draw
  fonts.rs       the Windows fonts with Arabic, Chinese, Japanese and Korean
  languages.rs   the languages, their samples and the code each service wants
  hover.rs       the thread that watches the pointer and asks for the text
  uia.rs         reading text with UI Automation (accessibility APIs)
  settings.rs    the preferences and where they are stored
  translate/
    mod.rs       the `TranslationProvider` trait, the shared helpers
    mymemory.rs  a free, key-less public service
    backend.rs   a backend relay that keeps the provider key on the server
    libretranslate.rs  a LibreTranslate instance
    cache.rs     a short lived in-memory cache
    ratelimit.rs a token bucket that protects the free quota
```

### Adding another service

Implement `TranslationProvider` in `src/translate/` and add it to
`build_provider()` in `src/app.rs`. Nothing else has to change: the panel, the
pointer watcher and the card only know the trait.

```rust
pub trait TranslationProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> String;
    fn translate(&self, request: &TranslationRequest) -> Result<TranslationOutcome, TranslateError>;
}
```

A new service should also say which language codes it wants; that is what
`languages::provider_target()` and `languages::provider_source()` are for.

## Where the settings live

`%APPDATA%\HoverTranslate\settings.json` holds the preferences only: the
switch, the delay, the target language, the source language, the chosen
service and its address. No hovered text, no translation and nothing about the
applications that were open is ever written to disk.

## Privacy and safety

- Only the text under the pointer is sent, and only when a translation is
  actually needed.
- Translations are cached in memory for a few minutes so the same sentence is
  not sent twice. The cache is never written to disk and disappears when the
  app closes.
- A rate limiter keeps the number of requests per minute under the value from
  the settings, so the free quota is not used up by accident.
- The public release should use the backend relay: the provider key then stays
  on the server and is never part of the `.exe`.

## Trying it out

`shots/` holds a few small helper scripts that were used to check the app by
hand on Windows: they open a window with real text, move the pointer onto it
and take a picture of the screen, so the hover → translation → card chain can
be watched in a real desktop session.

```sh
# a window with text in another language
powershell -File shots\form.ps1
# move the pointer onto that text and take a picture of the screen
powershell -File shots\grab3.ps1 -X 640 -Y 320
```

The app prints what it reads and what the service answers on standard output,
which makes it easy to see what is happening.

## The backend relay (optional)

`backend/server.py` is a small reference relay. It owns the provider key and
forwards the translation requests of the desktop app:

```sh
cd backend
set MYMEMORY_EMAIL=you@example.com
python server.py
```

The app then expects `POST /translate` with

```json
{ "text": "hello", "source": "auto", "target": "es" }
```

and answers with

```json
{ "translation": "hola", "detected_source": "en" }
```

Any other service (LibreTranslate, a paid API, a self-hosted model) can be
used behind the same small contract, which keeps the desktop app free of
secrets. The relay cuts long texts into pieces and takes the notices of the
free service off in the same way the desktop app does.

## License

MIT — see [LICENSE](LICENSE).
