# TypeUrVoice (VoiceInk)

A fast, private voice-to-text app for macOS (and Windows). Press a hotkey, speak, release — your words are transcribed and pasted into whatever app you're using. It lives in the menu bar, never steals focus, and can run **fully offline**.

Free and open source under the [MIT License](LICENSE).

> The app itself is still called **VoiceInk** inside the code and the built `.app`. It is not related to other projects with a similar name.

## Features

- **Global hotkey** — Hold or Toggle mode, plus a second hotkey for a second language
- **Local transcription** — whisper.cpp with Metal acceleration on Apple Silicon; models are downloaded on demand and SHA-256 verified
- **Cloud transcription** — OpenAI or Groq Whisper, if you prefer
- **Text cleanup** — Raw, Clean (grammar + filler removal), Polished, or your own custom prompt profiles
- **Many LLM backends** — built-in local model (llama.cpp sidecar), Ollama, LM Studio, OpenAI, Anthropic, Gemini, and OpenAI-compatible providers (Grok, Groq, DeepSeek, Mistral, OpenRouter, Perplexity, Together, custom URL)
- **Translation and summaries**, personal dictionary, history
- **Live preview** of the transcript while you speak
- **Knock to record** — double-knock on a MacBook's case to start/stop (Apple Silicon)
- **Private by default** — no telemetry; API keys stay on your machine with owner-only file permissions

## Three ways to run it

| Mode | Transcription | Text cleanup | What leaves your Mac |
|---|---|---|---|
| Fully offline | Local Whisper | Built-in model / Ollama / LM Studio | Nothing |
| Semi-offline | Local Whisper | Any cloud LLM | Transcript text only |
| Fully online | OpenAI / Groq Whisper | Any cloud LLM | Audio and text |

## Building from source

Requirements: macOS 12+, [Rust](https://rustup.rs) (stable, ≥ 1.99), Node.js 20+, and `cmake` (`brew install cmake`).

```bash
npm install
npm run tauri dev        # run in development
npm run dmg              # release build + DMG (also builds the llama-server sidecar)
```

Builds are ad-hoc signed by default. macOS ties the Accessibility permission (needed for auto-paste) to the signature, so ad-hoc builds lose it on every rebuild. If you have an Apple Developer certificate, sign with it instead:

```bash
export APPLE_SIGNING_IDENTITY="Apple Development: Your Name (TEAMID)"
npm run dmg
```

## Permissions

On first launch, grant **Microphone** and **Accessibility** (System Settings → Privacy & Security). Accessibility is only used to simulate Cmd+V for auto-paste; the text is always copied to the clipboard as well.

## Project layout

- `src-tauri/` — Rust backend (audio capture, transcription, LLM processing, paste, tray)
- `src/` — React + TypeScript settings UI, onboarding and overlay
- `CLAUDE.md` — detailed engineering notes and history of fixes

## Contributing

Issues and pull requests are welcome.

## License

[MIT](LICENSE) © 2026 Nicolosi corp a.s.
