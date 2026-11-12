import { ReactNode } from 'react'
import { Section } from '../ui/Section'

export function HelpTab() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--sp-5)' }}>

      {/* ── Quick start ── */}
      <Section title="Quick Start">
        <Para>
          TypeUrVoice lives in your menu bar. Press your shortcut, speak, release — your
          words appear wherever your cursor is.
        </Para>
        <Steps items={[
          'Pick a Transcription engine in the Engine tab (Cloud or Local).',
          'If Cloud: choose your Whisper host (OpenAI or Groq) and paste that provider\'s API key.',
          'If Local: download a Whisper model.',
          'Pick a Text Processing provider (OpenAI, Anthropic, Gemini, an OpenAI-compatible host, Ollama, or LM Studio) and paste its key.',
          'Choose an Output mode in the Output tab. Clean is the default — punctuation, grammar, basic filler removal.',
          'Place your cursor in any app, hold the shortcut, speak, release.',
        ]} />
        <Note>
          On first launch macOS will ask for <B>Microphone</B> and <B>Accessibility</B>
          permissions. Both are required: Microphone to capture audio, Accessibility to
          paste the result into other apps.
        </Note>
      </Section>

      {/* ── General tab ── */}
      <Section title="General Tab">
        <Field name="Primary Shortcut">
          The keyboard combination that starts a recording, plus the language you'll
          speak. Click the shortcut field and press your desired keys
          (e.g. <B>Option+Space</B>). The Language picker tells the transcriber what to
          listen for — pick one explicitly for short clips, since auto-detection is
          unreliable on a few seconds of audio.
        </Field>
        <Field name="Secondary Shortcut">
          A second shortcut bound to a different language. Useful if you switch between
          two languages — e.g. <B>Option+Space</B> for Czech and <B>Alt+Shift+Space</B>
          for English. Both shortcuts go through the same pipeline; only the language
          differs. If you speak a different language than the shortcut's (say, Czech on
          the English one) and the clip is longer than about 2 seconds, the local engine
          notices: it transcribes what you actually said and translates it into the
          shortcut's language with your text processor.
        </Field>
        <Field name="Microphone">
          Pick a specific input device or leave on <B>System default</B>. If the device
          you saved gets unplugged, TypeUrVoice will quietly fall back to the system
          default and warn you here.
        </Field>
        <Field name="Recording mode — Hold to Record">
          Hold the shortcut while you speak, release to stop. Best for short messages.
          Has a built-in 600 ms minimum so quick taps don't get cut off. The recording
          bubble appears the moment the microphone is actually hearing you — start
          talking when you see it. The recording
          follows the key itself: it keeps going as long as the key is down, even if
          macOS reports a release too early, and stops when you let go even if the
          release report never arrives. Recordings are capped at 5 minutes — the
          timer turns orange in the last 30 seconds.
        </Field>
        <Field name="Recording mode — Toggle">
          Press once to start, press again to stop. Best for longer dictation where
          you don't want to hold a key down.
        </Field>
        <Field name="Show live text while speaking">
          With the <B>Local</B> engine, your words appear in the recording bubble as
          you talk — the bubble grows to show the last two lines. It's a preview:
          the text that gets pasted comes from a final pass over the whole recording
          after you stop, so it can be slightly more accurate than what you saw.
          Turn it off to save a little battery on long dictations.
        </Field>
        <Field name="Type sentences while speaking">
          With the <B>Local</B> engine, each sentence goes into your text field as soon
          as you've finished it, already cleaned up and translated. A sentence counts
          as finished when it ends with a full stop, question mark or exclamation mark
          and has come out the same twice in a row; very long stretches without one
          are sent anyway. When you stop, the rest follows, and the whole dictation is
          on the clipboard. Each sentence is translated together with the ones before
          it, so the context isn't lost. Needs <B>Auto-paste</B>, and pauses while
          <B> Summary</B> is on. Keep the cursor where the text should go — clicking
          into another app sends the next sentence there. A sentence that's already
          typed can't be corrected afterwards, so for important text the single paste
          at the end is the most accurate.
        </Field>
        <Field name="Sound While Recording">
          <B>Pause videos and music</B>: if something is playing when you start
          recording — a YouTube video, Spotify, a podcast — TypeUrVoice presses the
          Play/Pause key, and presses it again when you stop, so it continues where it
          was. It only does this when an app is actually sending sound to your
          speakers, so a paused player is never started. A browser can keep reporting
          sound for a few seconds after you pause a video yourself; if a video ever
          starts instead of stopping, use Mute. Needs Accessibility, like auto-paste.
          <br /><B>Mute sound</B>: mutes your speakers or headphones while you record and
          unmutes them when you stop (videos keep playing silently). If they were
          already muted, they stay muted. macOS only.
        </Field>
        <Field name="Knock to Record">
          On Apple Silicon MacBooks you can start a recording by knocking on the laptop
          case: knock twice, speak, knock twice again. It works like Toggle mode, and
          the stopping knocks are cut out of the audio. <B>Language</B> picks what a
          knock records in: your primary or secondary shortcut's language (speak
          something else and the result is translated into it, just like the
          shortcut), <B>double knock = primary, triple knock = secondary</B>, or
          <B> Detect the language I speak</B> (no translation; short clips can be
          misdetected). In the double/triple mode it waits a moment after two knocks
          for a possible third, and either pattern stops the recording. Choose <B>Triple knock</B> if another app (such as Knock) already
          reacts to double knocks. Bumps while you type or click are ignored. Raise
          <B> Sensitivity</B> if gentle knocks are missed, lower it if recording
          starts on its own. The section only appears on Macs with the motion sensor
          (MacBook Air/Pro with Apple Silicon); desktop Macs and Windows don't have one.
        </Field>
        <Field name="Start on login">
          Launch TypeUrVoice automatically when you log in to your Mac.
        </Field>
        <Field name="Check for updates">
          Looks for a newer release. If updates are not yet active for your build,
          you'll see a "Check failed" message — that's expected, not a bug.
        </Field>
        <Field name="Verbose diagnostics">
          When ON, the local <B>crash.log</B> file includes short transcript previews,
          which makes bug reports much easier to triage. OFF by default for privacy.
          Only turn it on while debugging an issue.
        </Field>
        <Field name="Show crash.log">
          Opens the diagnostics file in your default text editor. Helpful when
          something goes wrong — the file shows what the app did, step by step.
        </Field>
      </Section>

      {/* ── Engine tab ── */}
      <Section title="Engine Tab — Pick Transcription + Text Processing Independently">
        <Para>
          TypeUrVoice has two independent stages: <B>Transcription</B> (speech → raw
          text) and <B>Text Processing</B> (cleanup, polish, translation, summary).
          You pick a backend for each. The example modes below are common
          combinations — but every transcription engine works with every text
          processor, so mix and match freely.
        </Para>

        <ModeCard
          tag="Fully Online"
          title="Cloud Whisper + Cloud LLM"
          summary="Easiest setup, best accuracy, requires internet."
          bullets={[
            'Audio is sent to OpenAI Whisper or Groq Whisper for transcription.',
            'Transcripts are sent to OpenAI / Anthropic / Gemini / etc. for cleanup.',
            'No model download, no local CPU/GPU work.',
            'Pay-per-use — typically pennies per minute.',
          ]}
          when="Pick this if you want best accuracy with least friction and don't mind audio leaving your device."
        />

        <ModeCard
          tag="Semi-Offline"
          title="Local Whisper + Cloud LLM"
          summary="Audio never leaves your Mac; only the transcript is sent to the cloud."
          bullets={[
            'Speech recognition runs entirely on your Mac.',
            'Cleanup / polish / translate / summarize use a cloud LLM (your choice).',
            'Audio is never uploaded — only the resulting text.',
            'Needs a downloaded local Whisper model + an LLM API key.',
          ]}
          when="Pick this when audio is sensitive (meetings, voice notes) but you still want a strong LLM to clean it up."
        />

        <ModeCard
          tag="Fully Offline"
          title="Local Whisper + Built-in model"
          summary="Nothing leaves your Mac. Works on a plane. No extra apps."
          bullets={[
            'Both transcription and cleanup run on your Mac\'s GPU.',
            'Pick Built-in (on this Mac) under Text Processing and download a small model (0.8–3 GB).',
            'No Ollama or LM Studio needed — TypeUrVoice runs the model itself.',
            'Ollama and LM Studio still work if you prefer your own setup.',
          ]}
          when="Pick this for maximum privacy or when offline. Set Output mode to Raw to skip the LLM step entirely."
        />

        {/* Transcription details */}
        <Field name="Transcription — Cloud provider (OpenAI or Groq)">
          <B>OpenAI gpt-transcribe</B> (default) is OpenAI's newest speech-to-text
          model — more accurate than the old whisper-1 and cheaper per minute.
          <B> gpt-4o-mini-transcribe</B> is cheaper still; <B>whisper-1</B> stays
          available as a legacy option. <B>Groq Whisper Large v3 Turbo</B> is very
          fast (often under a second) with a generous free tier. Pick the model
          under <B>Transcription model</B>. Each provider uses its own API key slot.
        </Field>
        <Field name="Transcription — Local (Whisper)">
          Runs Whisper on your Mac via whisper.cpp, accelerated by the Apple
          Silicon GPU (Metal). Fully offline once a model is downloaded. The model
          is loaded in the background when TypeUrVoice starts, so even the first
          recording is fast. Multilingual Q5 models (small/medium/large/turbo) handle
          Czech, Slovak, English and 90+ other languages. The English-only models
          (<B>tiny-en</B>, <B>base-en</B>) are smaller and faster but don't work for
          other languages.
        </Field>
        <Field name="Local Models">
          Click <B>Download</B> to fetch a model. Files are verified by SHA-256 to
          prevent tampering. Larger models = better accuracy but slower transcription
          and more disk space. <B>Large Turbo Q5</B> is the recommended default —
          the best Czech/Slovak accuracy, and fast on the GPU; <B>small-q5</B> if
          disk space is tight. The hash check runs once per model per app start
          (cached afterwards), not on every recording.
        </Field>

        {/* Text Processing — provider catalogue */}
        <Para>
          <B>Text Processing</B> picks the LLM that handles Clean / Polish /
          Translate / Summarize. Pick one from the dropdown:
        </Para>
        <Field name="Built-in (on this Mac)">
          TypeUrVoice runs a small AI model itself — no Ollama, LM Studio, account or
          API key. Download one of: <B>Qwen3.5 2B</B> (recommended, fast,
          multilingual), <B>Qwen3.5 0.8B</B> (tiniest), <B>Qwen3.5 4B</B> (best
          quality of the small ones), <B>Gemma 4 E2B</B> (strong on Czech/Slovak), or —
          on a Mac with 16 GB of memory or more — the larger <B>Gemma 4 E4B</B> and
          <B> Qwen3.5 9B</B> for the best Polished and Custom results.
          Every download is verified by SHA-256.
          <br /><br />
          <B>Load automatically and unload when idle</B> (default ON): the model
          starts loading the moment you press the shortcut — while you're still
          speaking — and frees its memory after a few idle minutes (you choose how
          many). Turn it OFF to keep the model loaded from launch until you quit:
          slightly faster, but it always uses 1–6 GB of RAM. <B>Unload now</B>
          frees the memory immediately. In Raw mode the model is never loaded.
        </Field>
        <Field name="OpenAI">
          <B>GPT-6 Luna</B> (default) is OpenAI's most efficient model for quick,
          high-volume tasks like cleanup. GPT-6.1 Terra (fast), GPT-6.1 Sol
          (balanced) and GPT-6 Astra (flagship) are also listed. TypeUrVoice asks reasoning models for minimal thinking so
          cleanup stays fast. Get a key at platform.openai.com.
        </Field>
        <Field name="Anthropic Claude">
          <B>Claude Haiku 5.5</B> (default), Sonnet 5.5, Opus 5.5 and Fable 5.1
          (the most capable, and the slowest). Excellent at
          following instructions and preserving the speaker's voice. TypeUrVoice uses
          low effort for fast cleanup. Get a key at console.anthropic.com.
        </Field>
        <Field name="Google Gemini">
          <B>Gemini 3.5 Flash-Lite</B> (default), Gemini 3.8 Flash, 3.6 Flash and
          Gemini 4 Argon. Retired models (Gemini 2.5, 3.5 Flash, 3.7 Flash) are moved
          to their replacements automatically. Get a key at
          aistudio.google.com. We set permissive safety thresholds — this is a
          dictation tool, the user is dictating their own words.
        </Field>
        <Field name="OpenAI-compatible (cloud)">
          A single dropdown that covers <B>seven</B> providers that all speak the
          OpenAI API shape: <B>xAI Grok</B>, <B>Groq</B> (a fast inference host —
          different from xAI Grok), <B>DeepSeek</B>, <B>Mistral</B>, <B>OpenRouter</B>
          (a meta-aggregator with 100+ models, one key), <B>Perplexity</B>,
          <B> Together AI</B>, plus a <B>Custom</B> option for any other URL. Each
          gets its own key slot — switching between providers does not delete your
          saved keys.
        </Field>
        <Field name="Ollama (local)">
          Sends transcripts to a local Ollama server (default
          <B> http://localhost:11434</B>). Run <Code>ollama serve</Code> and pull a
          model like <Code>gemma4</Code>, <Code>qwen3.5</Code> or <Code>ministral-3</Code>. Fully offline. No key
          needed. (Built-in is simpler if you don't already use Ollama.)
        </Field>
        <Field name="LM Studio (local)">
          Sends transcripts to LM Studio's local server (default
          <B> http://localhost:1234</B>). Open LM Studio, load a model, and start its
          local server. Fully offline. No key needed.
        </Field>

        {/* Keys + models */}
        <Field name="API keys">
          All keys are stored locally on your Mac in
          <B> ~/Library/Application Support/TypeUrVoice/keys.json</B> with owner-only
          file permissions (chmod 0600). Each provider has its own slot. Keys are
          never sent anywhere except the corresponding provider. Use
          <B> Test connection</B> next to each key field to validate it before saving.
        </Field>
        <Field name="Model dropdown + Refresh from provider">
          The Model dropdown shows a hand-curated default list per provider. Click
          <B> Refresh from provider</B> to fetch the live model list from the
          provider's <Code>/models</Code> endpoint. The result is cached locally for
          24 hours, so you don't hammer the provider on every settings open. New
          models from the provider show up without a TypeUrVoice update.
        </Field>
        <Field name="OpenRouter / Together / Custom">
          These three have catalogues too large to enumerate (hundreds of models),
          so the Model field is a free-form text input. Recommended models are
          listed below it; or use <B>Refresh from provider</B> to pull the full list.
        </Field>
      </Section>

      {/* ── Output tab ── */}
      <Section title="Output Tab">
        <Field name="Remove filler words">
          ON by default. Strips hesitation sounds — <I>um, uh, er, hmm, ehm, eee,
          ééé, mmm</I> — from every transcript, in every mode including Raw. It's a
          fast rule-based filter (no AI, no delay) that only removes standalone
          sounds, never real words like Czech <I>a</I>, <I>u</I> or <I>no</I>.
          Clean and Polished additionally remove phrases like "you know" and false
          starts.
        </Field>
        <Field name="Output mode — Raw">
          Pastes the transcript exactly as Whisper produced it. <B>Fastest</B> — no
          second LLM call. Pick this if you want the unmodified output.
        </Field>
        <Field name="Output mode — Clean (default)">
          Fixes grammar and punctuation, and removes obvious filler words ("um",
          "uh", "like" as a discourse marker, "you know"). Doesn't rephrase or
          restructure your sentences. Good default for messages and notes.
          A built-in check compares the result with what was transcribed: if the
          model swaps a word for a different one, the original word is put back,
          and if it rewrites too much (or adds commentary), the plain transcript
          is pasted instead. Your Dictionary words are always allowed.
        </Field>
        <Field name="Output mode — Polished">
          Rewrites your speech into professional prose. Removes filler words and
          restructures sentences. Best for emails and documents — but it <I>will</I>
          change your wording.
        </Field>
        <Field name="Output mode — Markdown">
          Turns what you say into a structured document — headings, bullet and
          numbered lists, <Code>code</Code> for commands and file names — ready to
          paste into a chat with an AI assistant, an issue tracker or notes. Talk
          normally and say what you want from the result as you go: <I>“make it a
          list”</I>, <I>“add a section about testing”</I>, <I>“leave the prices
          out”</I>. Those instructions are followed, not written into the text.
          Technical terms (design, landing page, API…) stay in English.
          <br /><B>Write the document in</B>: by default the language of the shortcut
          you pressed — dictate in Czech on your English shortcut and you get an
          English document in one step. Pick a language to always use that one.
          <br /><B>Structure</B>: leave empty and the AI picks headings that fit, or
          write your own (one per line, e.g. <Code>## Goal</Code>) or start from a
          preset. Sections you say nothing about are left out. The whole dictation is
          processed at once, so <B>Type sentences while speaking</B> pauses in this
          mode. Uses the text processor from the Engine tab; the built-in Gemma 4 E4B
          or an online model gives the best structure.
        </Field>
        <Field name="Output mode — Custom (profiles)">
          Save as many named prompt profiles as you want — for example
          <I> Email tone</I>, <I>Tweet under 280</I>, <I>Bullet points only</I>,
          <I> Strip emojis</I>. Click a profile chip to make it active, the pencil
          icon to rename, the trash icon to delete. Switching profile while Custom
          mode is selected takes effect on the very next recording. The tray menu
          (<B>Output Mode → Custom</B>) lists every profile too, so you can switch
          without opening Settings. Hard rules (preserve language, no code fences,
          no commentary) are appended automatically. Empty prompt falls back to Raw.
        </Field>
        <Field name="Auto-paste">
          When ON, TypeUrVoice simulates Cmd+V into whichever app has focus right after
          transcription. When OFF, the text is still copied to your clipboard — paste
          manually. Auto-paste needs Accessibility permission.
        </Field>
        <Field name="Show notification">
          Shows a small system notification with a preview when transcription
          completes. Errors are <B>always</B> shown regardless of this setting.
        </Field>
      </Section>

      {/* ── Translation tab ── */}
      <Section title="Translation Tab">
        <Field name="Source language">
          The language you'll be speaking. <B>Auto-detect</B> works on long clips but
          is unreliable on a few seconds — for short dictation, set this explicitly.
          Note: the per-shortcut Language pickers in the General tab override this
          when you use those shortcuts.
        </Field>
        <Field name="Enable translation">
          When ON, the transcript is translated into the target language before being
          pasted. Uses whichever Text Processing provider you have selected (OpenAI,
          Anthropic, Gemini, OpenAI-compatible, Ollama, or LM Studio).
        </Field>
        <Field name="Target language">
          The language to translate <I>into</I>. Common pair: speak Czech, paste
          English.
        </Field>
        <Field name="How translation stays true to you">
          The translator is told the text comes from speech recognition, so it looks
          for the word you most likely said instead of translating a misheard one
          literally. It keeps negations and questions intact — a Czech
          "Nebylo by lepší…?" becomes "Wouldn't it be better…?", not a negative
          statement. It also uses your notes and learned profile (Dictionary tab) and
          the translations you fixed in History. Small on-device models still miss
          some words; <B>Gemma 4 E4B</B> or <B>Qwen3.5 9B</B> translate Czech much
          better than the 2B models.
        </Field>
      </Section>

      {/* ── Summary tab ── */}
      <Section title="Summary Tab">
        <Para>
          Summarize mode condenses long recordings into structured output. Useful for
          meeting notes and voice memos.
        </Para>
        <Field name="Enable summary">
          Adds a summary step to the pipeline. The summary uses your selected Text
          Processing provider and is written in the same language as your transcript
          (or the target language if Translation is also ON).
        </Field>
        <Field name="Summary type — Key Points">
          Bulleted list of the main ideas.
        </Field>
        <Field name="Summary type — Action Items">
          Pulls out tasks and to-dos from the transcript.
        </Field>
        <Field name="Summary type — TL;DR">
          One short paragraph capturing the gist.
        </Field>
        <Field name="Summary shortcut">
          Optional dedicated shortcut that records and always runs the summary step,
          regardless of the global Output mode.
        </Field>
      </Section>

      {/* ── Dictionary tab ── */}
      <Section title="Dictionary Tab">
        <Field name="How You Speak — notes">
          Tell TypeUrVoice anything about your speech, e.g. "I can't pronounce ř — it
          sounds like ž". Cleanup and translation use the note to put back the words
          the recognizer got wrong (so "pozádku" becomes "pořádku" instead of
          staying misspelled).
        </Field>
        <Field name="Learn from my dictations">
          Every 8 dictations, your on-device model (Built-in, Ollama or LM Studio)
          reads your recent ones and updates a short profile: what you talk about,
          names and terms, words it mishears for you, and how you phrase things. The
          terms help Whisper spell them right, the rest helps cleanup and
          translation sound like you. With an online text processor it only updates
          when you click <B>Update now</B>, because that sends recent dictations to
          the provider. You can edit or clear the profile at any time; it's stored
          only on your Mac.
        </Field>
        <Para>
          The dictionary lists words and phrases that Whisper should bias toward.
          Helpful for names, jargon, product names, and acronyms that Whisper would
          otherwise mishear or spell phonetically.
        </Para>
        <Field name="Adding entries">
          Type a word or short phrase and press Enter. Add the exact spelling you
          want — capitalization matters for proper nouns.
        </Field>
        <Field name="What it does">
          Entries (plus the terms from your learned profile) are passed to Whisper as
          a prompt hint — with both the Local and Cloud engines. Whisper still has the
          final say — the dictionary is a nudge, not a rule. It works best for unique
          words (proper nouns) rather than common ones.
        </Field>
      </Section>

      {/* ── History tab ── */}
      <Section title="History Tab">
        <Field name="History list">
          Every transcription is saved locally to a SQLite database with the raw
          text, processed text, language, duration and timestamp. Click an entry to
          see what you said and its translation; the copy icon copies the output.
        </Field>
        <Field name="Fix a text (pencil)">
          Correct a cleanup or translation that came out wrong and click
          <B> Save fix</B>. TypeUrVoice remembers the fix: the next time similar
          words come up, the cleanup or translator gets your fix as an example, and
          the learned profile picks it up too. Clearing History also forgets your
          fixes.
        </Field>
        <Field name="Retention">
          History older than the chosen retention period is deleted automatically on
          app start. Set to a small number for tighter privacy.
        </Field>
        <Field name="Clear all">
          Two-click confirmation. Click once → "Confirm Clear?" appears for 3
          seconds. Click again to wipe history. Press Esc or wait 3 s to cancel.
        </Field>
      </Section>

      {/* ── Troubleshooting ── */}
      <Section title="Troubleshooting">
        <Field name="Recording produces no text">
          Most often the microphone is muted, dead, or TypeUrVoice doesn't have
          Microphone permission. Open <B>System Settings → Privacy &amp; Security →
          Microphone</B> and confirm TypeUrVoice is enabled. Open <B>Show crash.log</B>
          in General — a line like <Code>peak=0.0003</Code> means the mic captured
          silence.
        </Field>
        <Field name="Text doesn't paste">
          Auto-paste requires Accessibility permission. Open <B>System Settings →
          Privacy &amp; Security → Accessibility</B>, find TypeUrVoice, and toggle it
          on. The clipboard always gets the text even if pasting fails — Cmd+V
          manually.
        </Field>
        <Field name="Wrong language detected">
          For short clips, set <B>Source language</B> explicitly in the Translation
          tab, or use a per-shortcut language in the General tab.
        </Field>
        <Field name="Cleanup says “I can only assist with English text”">
          Means the LLM was told the wrong language. Set Source language explicitly;
          this is the same fix as above.
        </Field>
        <Field name="Knocking does nothing">
          Knock firmly with a knuckle on the palm rest or the case. Two knocks
          should be about a quarter to half a second apart, with a short pause
          before the first one. Make sure you're not typing at the same moment.
          Try <B>High</B> sensitivity. crash.log shows a <Code>Knock:</Code> line
          for every recognised pattern; <Code>Knock: listener failed</Code> means
          macOS refused access to the sensor.
        </Field>
        <Field name="App seems frozen mid-recording">
          The pipeline has a 360 s overall timeout (with 60 s per LLM call). If it's
          been stuck longer, quit from the tray menu and check crash.log for the
          last logged step.
        </Field>
        <Field name="Built-in model won't load">
          The status line under Built-in shows the reason. Most often the Mac is
          short on free memory — quit a few apps or pick a smaller model (Qwen3.5
          0.8B needs about 1 GB). If it says verification failed, delete the model
          and download it again. Details are in <Code>llama-server.log</Code> next to
          crash.log.
        </Field>
        <Field name="LLM connection failed / 429 errors">
          TypeUrVoice retries once on a 2-second backoff for transient errors (429 rate
          limit, 5xx). If you keep hitting them, you're either rate-limited (wait a
          minute) or the provider's down (try a different one). Auth errors (401/403)
          don't retry — re-paste your key.
        </Field>
        <Field name="My provider released a new model — where is it?">
          Open Engine → Text Processing → Model dropdown → click <B>Refresh from
          provider</B>. TypeUrVoice fetches the live list from the provider's
          <Code> /models</Code> endpoint and caches it for 24h.
        </Field>
      </Section>

      {/* ── Privacy ── */}
      <Section title="Privacy Notes">
        <Bullets items={[
          'All settings, history, models and API keys are stored locally on your Mac.',
          'API keys live in keys.json with chmod 0600 (owner-only). One slot per provider — switching providers does not leak any other key.',
          'In Cloud transcription mode, audio is sent to your chosen Whisper host (OpenAI or Groq). In Local mode, audio never leaves your machine.',
          'Text processing sends only transcripts — never audio — and only to the single provider you selected (OpenAI, Anthropic, Gemini, an OpenAI-compatible host, Ollama, or LM Studio).',
          'Built-in text processing runs entirely on your Mac: the model server listens only on 127.0.0.1 (not reachable from your network) and is stopped when TypeUrVoice quits.',
          'crash.log does not include transcripts unless Verbose diagnostics is ON.',
          'The 24-hour live model cache is stored locally (model_cache.json). It contains only model identifiers, no transcripts.',
          'TypeUrVoice has no analytics, telemetry, or accounts.',
        ]} />
      </Section>
    </div>
  )
}

// ── Sub-components ────────────────────────────────────────────────────────────

function Para({ children }: { children: ReactNode }) {
  return (
    <p style={{
      margin: 0,
      fontSize: 'var(--text-sm)',
      color: 'var(--text-secondary)',
      lineHeight: 1.55,
    }}>
      {children}
    </p>
  )
}

function Note({ children }: { children: ReactNode }) {
  return (
    <div style={{
      padding: '8px 10px',
      borderRadius: 'var(--radius)',
      border: '1px solid var(--border-subtle)',
      background: 'var(--bg-secondary)',
      fontSize: 'var(--text-sm)',
      color: 'var(--text-secondary)',
      lineHeight: 1.55,
    }}>
      {children}
    </div>
  )
}

function Field({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      <div style={{
        fontSize: 'var(--text-sm)',
        fontWeight: 500,
        color: 'var(--text-primary)',
      }}>
        {name}
      </div>
      <div style={{
        fontSize: 'var(--text-sm)',
        color: 'var(--text-secondary)',
        lineHeight: 1.55,
      }}>
        {children}
      </div>
    </div>
  )
}

function Bullets({ items }: { items: string[] }) {
  return (
    <ul style={{
      margin: 0,
      paddingLeft: 18,
      fontSize: 'var(--text-sm)',
      color: 'var(--text-secondary)',
      lineHeight: 1.55,
    }}>
      {items.map((it, i) => <li key={i} style={{ marginBottom: 3 }}>{it}</li>)}
    </ul>
  )
}

function Steps({ items }: { items: string[] }) {
  return (
    <ol style={{
      margin: 0,
      paddingLeft: 20,
      fontSize: 'var(--text-sm)',
      color: 'var(--text-secondary)',
      lineHeight: 1.55,
    }}>
      {items.map((it, i) => <li key={i} style={{ marginBottom: 3 }}>{it}</li>)}
    </ol>
  )
}

function ModeCard({ tag, title, summary, bullets, when }: {
  tag: string
  title: string
  summary: string
  bullets: string[]
  when: string
}) {
  return (
    <div style={{
      padding: '10px 12px',
      borderRadius: 'var(--radius-md)',
      border: '1px solid var(--border-subtle)',
      background: 'var(--bg-secondary)',
      display: 'flex',
      flexDirection: 'column',
      gap: 6,
    }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
        <span style={{
          fontSize: 10,
          padding: '1px 6px',
          borderRadius: 999,
          background: 'rgba(107, 139, 255, 0.18)',
          color: 'var(--border-focus)',
          fontWeight: 600,
          letterSpacing: '0.04em',
          textTransform: 'uppercase',
        }}>
          {tag}
        </span>
        <span style={{ fontSize: 'var(--text-sm)', fontWeight: 500, color: 'var(--text-primary)' }}>
          {title}
        </span>
      </div>
      <div style={{ fontSize: 'var(--text-sm)', color: 'var(--text-secondary)', lineHeight: 1.5 }}>
        {summary}
      </div>
      <ul style={{
        margin: 0,
        paddingLeft: 18,
        fontSize: 'var(--text-xs)',
        color: 'var(--text-secondary)',
        lineHeight: 1.5,
      }}>
        {bullets.map((b, i) => <li key={i}>{b}</li>)}
      </ul>
      <div style={{
        fontSize: 'var(--text-xs)',
        color: 'var(--text-tertiary)',
        fontStyle: 'italic',
        lineHeight: 1.5,
      }}>
        {when}
      </div>
    </div>
  )
}

function B({ children }: { children: ReactNode }) {
  return <strong style={{ color: 'var(--text-primary)', fontWeight: 500 }}>{children}</strong>
}

function I({ children }: { children: ReactNode }) {
  return <em style={{ fontStyle: 'italic' }}>{children}</em>
}

function Code({ children }: { children: ReactNode }) {
  return (
    <code style={{
      fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
      fontSize: '0.92em',
      padding: '0 4px',
      borderRadius: 3,
      background: 'var(--bg-active)',
      color: 'var(--text-primary)',
    }}>
      {children}
    </code>
  )
}
