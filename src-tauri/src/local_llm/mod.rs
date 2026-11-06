//! Built-in on-device text processing: GGUF models served by a bundled
//! `llama-server` sidecar (llama.cpp), spoken to over the same
//! OpenAI-compatible protocol as Ollama / LM Studio.
//!
//! Why a sidecar instead of linking llama.cpp: whisper-rs already vendors
//! ggml, and two statically linked ggml copies collide at link time. A
//! separate process also means a model crash can never take TypeUrVoice down —
//! the same reasoning as the osascript paste path.

pub mod models;
pub mod server;
