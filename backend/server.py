"""A tiny reference backend for HoverTranslate.

The desktop app never carries a provider key. It sends the text it needs to
this service, and the service talks to the translation provider. The key only
lives in the environment of this process.

Run it with:

    set MYMEMORY_EMAIL=you@example.com     # optional, raises the free quota
    python server.py

The app then expects a POST to /translate with a JSON body:

    {"text": "hello", "source": "auto", "target": "es"}

and answers with:

    {"translation": "hola", "detected_source": "en"}

A simple in-memory cache and a small rate limit keep the usage under control.
Nothing is written to disk.
"""

from __future__ import annotations

import html
import json
import os
import re
import threading
import time
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PROVIDER = os.environ.get("TRANSLATE_PROVIDER", "mymemory")
MEMORY_EMAIL = os.environ.get("MYMEMORY_EMAIL", "").strip()
PORT = int(os.environ.get("PORT", "8787"))
REQUESTS_PER_MINUTE = int(os.environ.get("REQUESTS_PER_MINUTE", "60"))
CACHE_SECONDS = int(os.environ.get("CACHE_SECONDS", "600"))

# The free service counts the request in bytes. An Arabic letter is two bytes
# and a Chinese letter three, so the text is cut into smaller pieces.
MAX_QUERY_BYTES = 450

_cache: dict[str, tuple[str, str | None]] = {}
_cache_lock = threading.Lock()
_call_times: list[float] = []
_call_lock = threading.Lock()


def _cache_get(key: str) -> tuple[str, str | None] | None:
    with _cache_lock:
        entry = _cache.get(key)
        if not entry:
            return None
        value, stored = entry
        if time.time() - stored > CACHE_SECONDS:
            _cache.pop(key, None)
            return None
        return entry


def _cache_put(key: str, value: str, detected: str | None) -> None:
    with _cache_lock:
        if len(_cache) > 500:
            _cache.clear()
        _cache[key] = (value, detected)


def _rate_limited() -> bool:
    now = time.time()
    with _call_lock:
        while _call_times and now - _call_times[0] > 60:
            _call_times.pop(0)
        if len(_call_times) >= REQUESTS_PER_MINUTE:
            return True
        _call_times.append(now)
    return False


def _is_cjk(ch: str) -> bool:
    code = ord(ch)
    return (
        0x3000 <= code <= 0x303F
        or 0x3400 <= code <= 0x4DBF
        or 0x4E00 <= code <= 0x9FFF
        or 0xF900 <= code <= 0xFAFF
        or 0xFF00 <= code <= 0xFFEF
    )


def _split_for_request(text: str, max_bytes: int) -> list[str]:
    """Cut a text into pieces the service will take."""
    text = text.strip()
    if not text:
        return []
    if len(text.encode("utf-8")) <= max_bytes:
        return [text]

    pieces: list[str] = []
    current = ""
    last_break = -1
    for ch in text:
        size = len(ch.encode("utf-8"))
        if current and len(current.encode("utf-8")) + size > max_bytes:
            cut = last_break if last_break > 0 else len(current)
            piece = current[:cut].strip()
            rest = current[cut:].lstrip()
            if piece:
                pieces.append(piece)
            current = rest
            last_break = -1
        if ch in ".!?。！？،؛؟\n" or ch.isspace() or _is_cjk(ch):
            last_break = len(current.encode("utf-8"))
        current += ch
    tail = current.strip()
    if tail:
        pieces.append(tail)
    return pieces


def _join_pieces(pieces: list[str]) -> str:
    out = ""
    for piece in pieces:
        if not out:
            out = piece
            continue
        previous = out[-1]
        following = piece[0]
        if _is_cjk(previous) or _is_cjk(following):
            out += piece
        else:
            out += " " + piece
    return out


def _strip_notice(text: str, source: str) -> str:
    """The free service sometimes answers with a notice about the word."""
    trimmed = text.strip()
    if not (trimmed.startswith("[") and trimmed.endswith("]")):
        return text
    inner = trimmed[1:-1]
    if ":" not in inner:
        return text
    _, after = inner.split(":", 1)
    after = after.strip()
    if not after:
        return text
    if after.casefold() == source.strip().casefold():
        return ""
    return after


def _provider_target(target: str) -> str:
    """Each service names the languages a little differently."""
    if PROVIDER == "libretranslate" and target in ("zh-CN", "zh-TW"):
        return "zh"
    return target


def _translate_mymemory(text: str, source: str, target: str) -> tuple[str, str | None]:
    lang_pair = f"{source if source != 'auto' else 'autodetect'}|{_provider_target(target)}"
    query = urllib.parse.urlencode({"q": text, "langpair": lang_pair})
    if MEMORY_EMAIL:
        query += "&de=" + urllib.parse.quote(MEMORY_EMAIL)
    url = "https://api.mymemory.translated.net/get?" + query
    with urllib.request.urlopen(url, timeout=15) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if payload.get("responseStatus", 200) >= 400:
        raise RuntimeError(payload.get("responseDetails") or "the service refused the request")
    data = payload.get("responseData", {})
    detected = data.get("detectedLanguage") or data.get("detectedSourceLanguage")
    return html.unescape(data.get("translatedText", "")), detected


def _translate_libre(text: str, source: str, target: str) -> tuple[str, str | None]:
    endpoint = os.environ.get("LIBRETRANSLATE_URL", "http://127.0.0.1:5000/translate")
    body = json.dumps(
        {
            "q": text,
            "source": source,
            "target": _provider_target(target),
            "format": "text",
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        endpoint, data=body, headers={"Content-Type": "application/json"}
    )
    with urllib.request.urlopen(request, timeout=15) as response:
        payload = json.loads(response.read().decode("utf-8"))
    detected = payload.get("detectedLanguage", {})
    return payload.get("translatedText", ""), detected.get("language")


def translate(text: str, source: str, target: str) -> tuple[str, str | None]:
    key = f"{source}|{target}|{text}"
    cached = _cache_get(key)
    if cached is not None:
        return cached
    if _rate_limited():
        raise RuntimeError("too many requests")

    pieces = _split_for_request(text, MAX_QUERY_BYTES)
    answers: list[str] = []
    detected: str | None = None
    for piece in pieces:
        if PROVIDER == "libretranslate":
            result, language = _translate_libre(piece, source, target)
        else:
            result, language = _translate_mymemory(piece, source, target)
        result = _strip_notice(result, piece)
        if result:
            answers.append(result)
        if detected is None and language:
            detected = language

    translation = _join_pieces(answers)
    if not translation:
        raise RuntimeError("no translation")
    _cache_put(key, translation, detected)
    return translation, detected


class Handler(BaseHTTPRequestHandler):
    def _send(self, status: int, payload: dict) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - name fixed by the base class
        if self.path.rstrip("/") == "/health":
            self._send(200, {"status": "ok"})
        else:
            self._send(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802 - name fixed by the base class
        if self.path.rstrip("/") != "/translate":
            self._send(404, {"error": "not found"})
            return
        length = int(self.headers.get("Content-Length", "0"))
        try:
            request = json.loads(self.rfile.read(length).decode("utf-8"))
            text = str(request.get("text", "")).strip()
            source = str(request.get("source", "auto"))
            target = str(request.get("target", "es"))
            if not text:
                self._send(400, {"error": "no text"})
                return
            result, detected = translate(text, source, target)
            self._send(200, {"translation": result, "detected_source": detected})
        except RuntimeError as error:
            self._send(429, {"error": str(error)})
        except Exception as error:  # noqa: BLE001 - report anything to the client
            self._send(502, {"error": str(error)})

    def log_message(self, fmt: str, *args) -> None:
        print("[backend] " + fmt % args)


if __name__ == "__main__":
    print(f"[backend] listening on http://127.0.0.1:{PORT} using {PROVIDER}")
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
