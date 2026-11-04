# Implementation Plan — Central Europe & Poland Push

**Companion to:** `ROADMAP_CE_POLAND.md`
**Owner:** Tom
**Last updated:** 2026-05-11

This document translates the strategic roadmap into a concrete, engineering-level plan. It covers three workstreams that run in parallel:

1. **Localization** — UI + onboarding in Czech, Slovak, Polish
2. **Occupation Profile System** — pre-built, language-aware profiles with code-switching support (the headline feature)
3. **Market-fit polish** — locale-aware formatting, professional workflows, privacy mode

The plan is sequenced so we can ship a CE/PL beta in ~8 weeks without breaking the existing English/multi-provider experience.

---

## Table of Contents

1. [Guiding Principles](#guiding-principles)
2. [Workstream A — Localization (i18n)](#workstream-a--localization-i18n)
3. [Workstream B — Occupation Profile System](#workstream-b--occupation-profile-system)
4. [Workstream C — Locale-Aware Output](#workstream-c--locale-aware-output)
5. [Workstream D — Professional Workflows](#workstream-d--professional-workflows)
6. [Workstream E — Pricing & Packaging](#workstream-e--pricing--packaging)
7. [Workstream F — Launch Readiness](#workstream-f--launch-readiness)
8. [Sequencing & Milestones](#sequencing--milestones)
9. [Risk Register](#risk-register)
10. [Open Questions](#open-questions)

---

## Guiding Principles

These constraints govern every decision below. If a proposal violates one, push back.

1. **No English-market regression.** Existing users must see zero behavioural change unless they opt in to a new locale or profile.
2. **Settings schema stays backwards-compatible.** Every new field gets `#[serde(default)]` on the Rust side and a matching default on the TS side. Existing `settings.json` files must load cleanly.
3. **Dictionaries are hints, not laws.** Whisper biasing is probabilistic. We surface this honestly in UI copy.
4. **Code-switching is the real moat.** Czech/Slovak/Polish professionals constantly mix English technical terms into their native speech ("naimportuj ten CSV do BigQuery", "pacient má positive PCR test"). Competitors either butcher these (translate them) or refuse to clean them up. We solve this with explicit prompt rules + a preserve-list per profile.
5. **Privacy story stays sellable.** Every new feature must work in Fully Offline mode (local Whisper + Ollama/LM Studio). If a feature requires cloud, it's optional.
6. **Curated quality > community quantity.** Dictionary packs are reviewed by a domain expert before they ship. Bad medical terminology in a doctor's transcription is a customer-loss event, not a quirky bug.

---

## Workstream A — Localization (i18n)

### A.1 Architecture choice

**Decision:** Use `react-i18next` for the frontend, plain Rust string maps for backend-emitted strings.

- The settings UI is ~95% of user-visible text — frontend is the right place to put i18n.
- A small set of strings are emitted from Rust (notifications, error messages, tray menu items). These get a `LocaleStrings` struct keyed off the active locale, loaded once at startup.
- No live language switching for v1 — locale is read at startup from settings; changing it requires an app restart (with a clear toast: "Restart to apply language"). Live-switching React + tray + notifications is a rabbit hole we don't need to enter for the launch.

### A.2 Settings additions

```rust
// state.rs
pub enum AppLocale {
    En,  // existing behaviour
    Cs,  // Czech
    Sk,  // Slovak
    Pl,  // Polish
}

pub struct Settings {
    // ...
    pub app_locale: AppLocale,  // #[serde(default)] → En
}
```

Default resolution at first run:
- Read `sys-locale::get_locale()` (already a transitive dep via Tauri)
- If it starts with `cs` → `Cs`, `sk` → `Sk`, `pl` → `Pl`, else `En`
- User can override in General → Language

### A.3 Translation file structure

```
src/locales/
  en.json        # source of truth, English
  cs.json        # Czech
  sk.json        # Slovak
  pl.json        # Polish
```

JSON keyed by feature area:

```json
{
  "general": {
    "shortcut": {
      "primary": "Primary shortcut",
      "secondary": "Secondary shortcut",
      "captureHint": "Press the shortcut combination..."
    },
    "recordingMode": {
      "hold": "Hold",
      "toggle": "Toggle",
      "holdDescription": "Hold the key while speaking"
    }
  },
  "engine": { ... },
  "output": { ... },
  "translation": { ... },
  "summary": { ... },
  "dictionary": { ... },
  "history": { ... },
  "help": { ... },
  "errors": {
    "noSpeech": "No speech detected. Check your microphone and source language.",
    "noSpeechDeadMic": "Microphone is silent. Grant Microphone permission in System Settings."
  },
  "notifications": {
    "recordingFailed": "VoiceInk — recording failed",
    "transcribed": "Transcribed"
  }
}
```

### A.4 Translator workflow

- **English source** lives in the repo; treated as canonical.
- **Translation review:** native-speaker review for each of CS/SK/PL before shipping. Tom for CS, paid native reviewer for SK and PL (estimate €200 each for a one-time pass over ~800 keys).
- **Keyboard hint localization:** show `Alt+Mezerník` on Czech keyboards (not `Alt+Space`). Build a `formatShortcut(combo, locale)` helper that maps key names per locale.

### A.5 Backend-emitted strings

The Rust side touches user-visible strings in a small number of places:

- `tray.rs` — menu items ("Output Mode", "Quit VoiceInk", etc.)
- `lib.rs` — error notifications via `show_error_notification`
- `transcription/*.rs` — error messages bubbled to the UI

Approach: introduce `src-tauri/src/locale.rs` with a static lookup:

```rust
pub fn t(key: &str, locale: AppLocale) -> &'static str {
    match locale {
        AppLocale::Cs => CS_STRINGS.get(key).unwrap_or(&EN_STRINGS[key]),
        // ...
    }
}
```

Strings embedded via `include_str!` from `src-tauri/locales/cs.toml` etc. The set is small (~30 keys); a TOML file per locale is more readable than a HashMap literal.

### A.6 Onboarding localization

Onboarding is the most visible piece of UI for first-run impressions. Translate fully, but also:

- **Locale-detect on first launch.** If `sys-locale` returns Czech, the onboarding opens in Czech. No "pick your language" gate — friction kills first-run.
- **Mic permission copy** branches on locale AND engine (already branches on engine — Phase 3 work).

### A.7 Acceptance criteria

- [ ] Setting `app_locale: cs` and restarting changes 100% of visible UI to Czech
- [ ] Setting back to `en` works cleanly (no caching bugs)
- [ ] Tray menu, notifications, settings window, onboarding all consistent
- [ ] Czech keyboard shortcut display uses Czech key names
- [ ] No untranslated string remains in any locale's golden screenshot test

---

## Workstream B — Occupation Profile System

This is the differentiator. Everything else is table stakes; this is what makes a Czech lawyer pay €49.

### B.1 Concept

An **occupation profile** is a bundle of three things:

1. **Cleanup prompt** — tells the LLM what to do with the transcript
2. **Preserve list** — words/phrases that must NOT be translated or modified, even if they're English in a Czech sentence
3. **Dictionary entries** — proper nouns and jargon for Whisper biasing

Profiles are language-scoped: "Lékař (CS)" is a different profile from "Lekár (SK)" even though the occupation is identical.

### B.2 Data model

Extend the existing `CustomProfile`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OccupationProfile {
    pub id:              String,
    pub name:            String,       // "Lékařská zpráva", "Email klientovi", ...
    pub locale:          AppLocale,    // primary language of the speaker
    pub occupation:      Occupation,   // for grouping in UI
    pub prompt:          String,       // cleanup prompt template
    pub preserve_terms:  Vec<String>,  // English/foreign terms to keep verbatim
    pub dictionary:      Vec<DictEntry>,  // Whisper bias terms
    pub builtin:         bool,         // shipped vs. user-created
    pub version:         u32,          // for updateable built-ins
}

pub enum Occupation {
    Law,
    Medicine,
    Accounting,
    Marketing,
    Software,
    GeneralBusiness,
    Custom,
}

pub struct DictEntry {
    pub term:        String,
    pub pronunciation: Option<String>,  // optional phonetic hint
}
```

`CustomProfile` (the existing single-prompt struct from May 2026) is **deprecated** but not removed. Migration: when an `OccupationProfile` list exists and is non-empty, the old `custom_profiles` is treated as legacy/read-only. Users can manually re-create their custom prompts in the new system.

### B.3 The code-switching prompt rule

This is the technical core of the moat. The cleanup prompt for every CE/PL occupation profile includes a hard rule:

```
RULES:
1. The speaker is dictating in {LANGUAGE}. The output MUST be in {LANGUAGE}.
2. The speaker WILL mix English technical terms into their {LANGUAGE} sentences.
   This is normal. Do NOT translate these terms. Keep them in English, exactly
   as the speaker said them. Examples of terms that must stay in English:
   {PRESERVE_TERMS}
3. Apply {LANGUAGE} grammar rules to the surrounding {LANGUAGE} text:
   - Czech: comma before subordinate clauses, vyjmenovaná slova i/y, gender agreement on past tense verbs
   - Slovak: rytmický zákon, comma rules similar to Czech
   - Polish: comma before subordinate clauses, gendered past tense, ą/ę distinctions
4. Do NOT change the speaker's vykání/tykání (formal/informal you). Detect from
   first sentence and keep it consistent.
5. Strip fillers: "ehm", "no", "jakoby", "vlastně", "prostě" (CS); equivalents in SK/PL.
6. Do NOT add commentary, notes, or explanations. Output ONLY the cleaned text.
```

`{PRESERVE_TERMS}` is filled in at request time from the profile's preserve list PLUS auto-detected English nouns in the raw transcript (any word matching `[A-Z][A-Za-z]{2,}` that's not in the language's common-words list).

### B.4 Built-in profiles to ship at launch

We ship **24 profiles** at v1 (3 languages × 8 occupations). Names below are in the locale; the English description is for our reference.

#### Czech (CS)

| Profile name | Occupation | Use case |
|---|---|---|
| Lékařská zpráva | Medicine | Structured medical letter (anamnéza, vyšetření, závěr, doporučení) |
| Recept / předpis | Medicine | Short prescription notes, drug names preserved (Latin + brand names) |
| Soudní podání | Law | Formal legal filing, paragraph numbering, full dates |
| Email klientovi | Law / General | Formal CZ business letter with proper salutations |
| Faktura — popis | Accounting | Short, formal invoice line item |
| Účetní poznámka | Accounting | Internal accounting note, Pohoda/Money S3-friendly |
| Marketing copy | Marketing | Casual CZ marketing text with English brand/product terms preserved |
| Slack zpráva | Software / General | Casual, CZ/EN mix, no formal openers |

#### Slovak (SK)
Same 8 occupations, names adapted: Lekárska správa, Recept, Súdne podanie, Email klientovi, Faktúra — popis, Účtovná poznámka, Marketingový text, Slack správa.

#### Polish (PL)
Lekarska notatka, Recepta, Pismo procesowe, Email do klienta, Faktura — opis, Notatka księgowa, Tekst marketingowy, Wiadomość Slack.

### B.5 Preserve lists per occupation

This is the part competitors do not have. A few examples of what each profile's preserve list contains:

**Medicine (CS/SK/PL):**
```
PCR, MRI, CT, EKG, EEG, CRP, BMI, HDL, LDL, INR, BNP, COVID, RT-PCR,
positive, negative, anamnesis, status praesens, ad usum,
diagnosis, follow-up, baseline, screening, biopsy, stent,
plus all Latin drug substances (paracetamol, ibuprofen, metformin, ...)
```

**Law (CS/SK/PL):**
```
B2B, B2C, SaaS, EULA, NDA, GDPR, AML, KYC, IPO, M&A, due diligence,
compliance, escrow, fiduciary, on-prem, SLA, force majeure,
discovery, deposition, settlement (when used as English term)
```

**Accounting (CS/SK/PL):**
```
SaaS, ARR, MRR, churn, EBITDA, P&L, cashflow, runway, burn rate,
invoice, dashboard, KPI, OKR, finalize, audit, reconcile,
plus product names: Pohoda, Money S3, ABRA, ERP, SAP, NetSuite
```

**Marketing (CS/SK/PL):**
```
A/B test, CTR, CPC, CPM, CAC, LTV, funnel, landing page, copy,
hook, brand, campaign, retargeting, SEO, SEM, CRO,
performance marketing, conversion, drip campaign, newsletter,
plus tool names: Google Ads, Meta Ads, HubSpot, Mailchimp, Notion, Figma
```

**Software (CS/SK/PL):**
```
PR, MR, CI/CD, CD, deploy, deployment, rollback, hotfix, prod, staging,
merge, rebase, commit, branch, fork, repo, monorepo, microservice,
endpoint, schema, migration, refactor, lint, test, mock, stub,
plus tool/service names: GitHub, GitLab, Slack, Linear, Jira, AWS,
GCP, Azure, Kubernetes, Docker, Terraform, React, Rust, Python, ...
```

### B.6 Dictionary packs

The dictionary packs from ROADMAP §1.1 ship as JSON files bundled in the app:

```
src-tauri/dictionaries/
  cs/
    medicine.json   (~800 terms — drug names, MKN-10 abbreviations, anatomy)
    law.json        (~500 terms — court abbreviations, legal Latin, common parties)
    accounting.json (~400 terms — chart-of-accounts terms, DPH terminology)
    marketing.json  (~300 terms — common Czech marketing/agency vocab)
  sk/  (same structure)
  pl/  (same structure)
```

Format:

```json
{
  "language": "cs",
  "occupation": "medicine",
  "version": 1,
  "curated_by": "MUDr. <name>",
  "entries": [
    { "term": "antikoagulace", "pronunciation": null },
    { "term": "fibrilace síní", "pronunciation": null },
    { "term": "MKN-10", "pronunciation": "em-ká-en deset" },
    ...
  ]
}
```

When a user activates "Lékařská zpráva (CS)", the bundled `cs/medicine.json` is loaded and merged with the user's personal dictionary for Whisper biasing.

### B.7 UI

**Settings → Output Tab** gets a new section above Custom Profiles: **"Profesní profily" (Professional Profiles)**.

- Filter dropdown: by occupation, by language
- Each profile card shows: name, language flag, preview of cleanup behaviour, "Activate" button
- Active profile gets a green border + checkmark
- "Show prompt" expander — full transparency about what we send to the LLM
- "Edit copy" button — clones the profile as a custom one the user can modify

**Tray menu** — the existing "Custom" submenu also lists active occupation profile. When an occupation profile is active, the tray shows e.g. "✓ Lékařská zpráva (CS)" instead of just "Custom".

### B.8 Updating built-in profiles

Built-in profiles have a `version: u32`. When the app updates and a built-in's version is higher than the installed copy, the app silently upgrades it (preserving the user's clone if they've edited it under a different id).

User-created clones of built-ins are stored with `builtin: false` and are never overwritten.

### B.9 Acceptance criteria

- [ ] All 24 profiles install on fresh app launch with no setup
- [ ] Activating "Lékařská zpráva" + speaking "pacient má positive PCR test od soboty" produces output that keeps "positive PCR test" in English while making the rest grammatical Czech
- [ ] Activating "Marketing copy (CS)" + speaking "spustili jsme novou A/B test campaign na landing page" produces output keeping "A/B test", "campaign", "landing page" in English
- [ ] Profile switch from tray takes effect on the next recording (no restart)
- [ ] Bundled dictionaries auto-merge with personal dictionary when profile is active
- [ ] Editing a built-in profile creates a clone; original stays untouched
- [ ] Test rig: 30 representative clips per language, validated against a reference output

---

## Workstream C — Locale-Aware Output

### C.1 Number, date, and currency formatting

After cleanup, optionally re-format numbers, dates, currencies to local conventions.

**Decision:** This is a post-LLM string transform, not a prompt rule. LLMs are unreliable at consistent locale-formatting. We use regex + ICU.

```rust
// processing/locale_format.rs
pub fn format_locale(text: &str, locale: AppLocale) -> String {
    // 1. Detect numbers with format `\d{1,3}(,\d{3})*(\.\d+)?` (US-style)
    // 2. Reformat per locale:
    //    - cs: "1 234,56"   (NBSP thousands, comma decimal)
    //    - sk: "1 234,56"   (same)
    //    - pl: "1 234,56"   (same)
    //    - en: unchanged
    // 3. Currency symbols: "$100" → "100 Kč" etc. ONLY if speaker actually said
    //    "dollars" or similar; otherwise leave as-is
    // 4. Dates: "11/12/2026" → "12. 11. 2026" (cs/sk) or "12.11.2026" (pl)
}
```

**Toggle:** Settings → Output → "Auto-format numbers and dates for my language" (default: ON when locale ≠ En).

**Edge cases:**
- Phone numbers stay untouched (don't reformat `+420 123 456 789`)
- Code blocks / monospace runs stay untouched (won't appear from Whisper but defensive)
- The speaker explicitly says "dollars" or "eur" → currency conversion is NOT performed; only the symbol formatting

### C.2 Quotation marks

CS/SK use „lower opening" upper closing" (German-style guillemets). PL uses „...". English uses "...". Replace per locale, but only when we're confident it's a quotation (not e.g. inch marks).

---

## Workstream D — Professional Workflows

### D.1 App-specific paste verification

Manual verification matrix for the launch:

| App | macOS | Windows | Notes |
|---|---|---|---|
| Microsoft Word 365 | ✅ verify | ✅ verify | Critical for legal/medical |
| Microsoft Outlook | ✅ verify | ✅ verify | Critical for B2B |
| Apple Mail | ✅ verify | n/a | |
| Notes | ✅ verify | n/a | |
| Pohoda ERP | n/a | ✅ verify | Czech accounting standard |
| Money S3 | n/a | ✅ verify | Czech accounting |
| Datová schránka (web) | ✅ verify | ✅ verify | CZ legal mailbox; web UI |
| Slack desktop | ✅ verify | ✅ verify | |
| Notion | ✅ verify | ✅ verify | |
| Linear | ✅ verify | ✅ verify | |
| ISDOC viewer | n/a | ✅ verify | CZ invoice format viewer |
| LibreOffice Writer | ✅ verify | ✅ verify | Many CZ govt installs |

Failures get either (a) a workaround documented in Help, or (b) an app-specific fix path (e.g. a different paste method).

### D.2 Privacy Mode badge

One-click toggle in the menu bar dropdown: **"Plně offline režim" / "Fully offline mode"**.

When ON:
- Forces `engine = Local`
- Forces `text_processor ∈ {Ollama, LmStudio}`
- Adds a green dot to the menu bar icon
- Disables (greys out) cloud provider selectors with a tooltip: "Disabled in offline mode"

When OFF: returns to whatever the user had before.

State is stored as `privacy_mode_active: bool` plus a `pre_privacy_mode_snapshot: Option<Settings>` so toggling off restores the user's previous choices exactly.

### D.3 Long-recording mode

Current cap is 5 minutes. For dictating a medical report or a long legal filing, this is too short.

**Add:** Settings → General → "Recording duration limit": 5 / 10 / 30 minutes (default 5).
- 30 minutes requires Local engine (cloud cost guard)
- Buffer cap recomputed against the new limit
- UI warns about RAM usage at 30-minute setting

---

## Workstream E — Pricing & Packaging

Mirrors ROADMAP §3 but adds the technical packaging side.

### E.1 License key system

- License key issued by Paddle webhook → our minimal license server (one Cloudflare Worker + KV store)
- Key format: `VINK-XXXX-XXXX-XXXX-XXXX` (16 alphanumeric, hyphen-separated)
- Activation: app POSTs key + device fingerprint to our server, gets back signed entitlement JSON (RSA-signed, 30-day TTL, then renew)
- **Offline activation supported:** activation server returns a 1-year-TTL signed entitlement on first activation. After that, no server contact needed. Daily silent re-check is best-effort; failures don't disable the app.
- Device fingerprint = SHA-256 of (`hostname + MAC of primary interface + OS user id`). Stable across reboots, not tied to hardware that changes.

### E.2 Tier gating

| Feature | Free trial | Personal | Pro | Team |
|---|---|---|---|---|
| Local Whisper | ✅ | ✅ | ✅ | ✅ |
| Cloud Whisper | ✅ | ✅ | ✅ | ✅ |
| All LLM providers | ✅ | ✅ | ✅ | ✅ |
| Custom profiles | ✅ | 3 max | unlimited | unlimited |
| Built-in occupation profiles | ✅ (all) | 1 occupation only | all | all |
| Privacy mode | ✅ | ✅ | ✅ | ✅ |
| History retention | 7 days | 30 days | unlimited | unlimited |
| Device count | 1 | 1 | 3 | 10 |

**Free trial:** 14 days, full feature access. After expiry, app downgrades to a "Free tier" (Local engine + Raw mode only + 30 daily transcriptions). Not a hard wall — keeps the app useful so users come back to upgrade.

### E.3 Dictionary pack add-ons

Phase 5+ revenue stream. Sold separately at €19 each:

- **Právník Pack (CS)** — 5 000+ legal terms (extends bundled law.json with curated court abbreviations, judge names per region, more Latin)
- **Lékař Pack (CS)** — extended MKN-10, ATC codes, full SÚKL drug database
- **Účetní Pack (CS)** — full chart of accounts from Czech standards, DPH terminology corpus

Pack format = same `cs/medicine.json` schema. Delivered as signed JSON. License-gated.

---

## Workstream F — Launch Readiness

### F.1 Telemetry (opt-in, anonymized)

We currently have zero telemetry. For launch we add **minimal** opt-in telemetry:

```rust
pub enum TelemetryEvent {
    AppStart { version: String, os: String, locale: String },
    TranscriptionCompleted { engine: String, duration_ms: u32, success: bool },
    ProfileActivated { occupation: String, locale: String },
    Error { code: String },  // never includes user content
}
```

- Default: OFF.
- Setting: General → "Help improve VoiceInk" toggle.
- Endpoint: self-hosted, no third party.
- Never includes transcript content, file paths, or PII.
- Privacy policy explicitly enumerates what's sent.

### F.2 Crash reporting

Same as telemetry: opt-in, our own endpoint, redacts transcripts via the existing `redact_preview` helper.

### F.3 Auto-update activation

Phase 5 followup #1 from CLAUDE.md becomes blocking for launch:

- Generate Ed25519 keypair, store securely (1Password or similar)
- Public key into `tauri.conf.json::plugins.updater.pubkey`
- Flip `active: true`
- GitHub Actions workflow that signs `.app.tar.gz` + `.msi` and publishes `latest.json` on release

### F.4 Notarization & signing

- macOS: Apple Developer account + Developer ID Application cert ($99/yr — already noted)
- Windows: Code-signing cert (Sectigo or DigiCert, ~$200/yr). Without this, SmartScreen blocks every download.

### F.5 Localized landing pages

- `<name>.app/cs/`, `/sk/`, `/pl/`, `/en/` (default)
- Same content, manually translated and reviewed
- Hreflang tags so Google indexes them per market
- Pricing in local currency via geo-IP (CZK on `.cz` visitors, etc.) — display only, all charges in EUR

---

## Sequencing & Milestones

**Total: ~14 weeks from start to public launch.** Aggressive but doable solo if other projects are paused.

### Week 1 — Phase 0 blockers
- Rename, domain, GitHub org, bundle id (ROADMAP §0.1)
- Paddle setup, EU VAT OSS registration (§0.2, §0.3)
- s.r.o. vs OSVČ decision finalized

### Weeks 2–4 — Workstream B (Occupation Profiles)
- Data model + migration from `CustomProfile`
- 8 CS profiles drafted (prompts + preserve lists)
- 8 SK profiles
- 8 PL profiles
- Bundled dictionaries (start with medicine + law + accounting, marketing for v1.1)
- Test rig with 30 clips per language

### Weeks 3–5 — Workstream A (i18n) — parallel
- react-i18next integration
- en.json complete
- cs.json drafted (Tom)
- sk.json + pl.json contracted to reviewers
- Backend locale strings
- Onboarding in 4 languages

### Weeks 5–7 — Workstream C + D (locale formatting, professional workflows)
- Number/date/currency formatter
- Quotation marks
- App-specific paste matrix verification
- Privacy Mode toggle
- Long-recording mode

### Weeks 7–9 — Workstream E (licensing, pricing)
- License server (Cloudflare Worker)
- Tier gating in app
- 14-day trial flow
- Paddle integration end-to-end
- CZ invoice with DIČ verified

### Weeks 9–11 — Workstream F (launch readiness)
- Auto-update activation (Ed25519 keys + GH Actions release)
- macOS notarization, Windows signing
- Telemetry + crash reporting (opt-in)
- Localized landing pages

### Week 12 — Soft launch
- 50 hand-picked Czech users (Tom's network)
- Daily bug-fix cycle for a week
- Profile prompt tuning based on real recordings

### Week 13 — Press outreach (embargoed)
- Lupa.cz, Živě.cz, Root.cz pitches
- Free Pro keys to journalists

### Week 14 — Public launch (CZ)
- Press goes live
- Reddit r/Czech, r/MacApps
- LinkedIn CZ + EN

### Weeks 15–16 — SK + PL launch, HN, Product Hunt

---

## Risk Register

| Risk | Mitigation |
|---|---|
| Translation quality is uneven across SK/PL | Pay native reviewers; do a final pass against the test rig before public launch |
| Code-switching prompt fails on niche jargon | Test rig + preserve-list is per-profile; we can ship updated profiles via the version bump mechanism without an app update for users with auto-update enabled. Until updater is live, ship monthly app updates |
| LLM costs blow up if users default to GPT-4 | Default cleanup model stays gpt-4o-mini / Haiku 4.5 / Gemini Flash. UI warns when picking expensive models |
| Dictionary packs leak (sold pack copied across users) | Signed JSON tied to license id; refuse to load unsigned or mismatched packs. Best-effort — DRM is not the point, professional users won't bother |
| Whisper auto-detect picks SK on a CS recording | Already mitigated by primary/secondary shortcut + active-language override. New: profile activation also forces source_language |
| Pohoda/Money paste fails on Windows | Verify in week 6. Fallback: ship a one-click clipboard-only mode for those specific apps (no auto-paste, user presses Ctrl+V manually) |
| User hits the 14-day trial wall on day 12 | We send a soft email at day 10 + in-app banner at day 12. Soft downgrade (not hard lock) at day 14 keeps them in the funnel |

---

## Open Questions

1. **Profile prompt review.** Who validates that the medical-CS prompt produces good output? Need a friendly MD willing to do a 1-hour test session before each language goes live.
2. **Dictionary curation.** Do we hire a part-time CZ medical professional to maintain the medicine dictionary, or scrape from public sources (SÚKL, MKN-10) and review ourselves? €/quality tradeoff.
3. **Slovak market size.** Do we treat SK as a first-class market or as a free bonus for Czech-speaking users? Affects whether SK gets its own landing page from day 1 (recommended yes).
4. **Open-sourcing the dictionaries.** Bundled dictionaries (medicine.json etc.) — open them up, or keep them as a competitive moat? Recommendation: open the v1 dictionaries (good PR, helps the ecosystem) and keep the paid Pro Packs (5x larger, curated) closed.
5. **Polish reviewer trustworthiness.** We don't have a Polish network. Where do we find a Polish native who can review 800 UI strings + 8 occupation prompts? Upwork, or Czech friend's recommendation chain?
6. **Telemetry endpoint.** Self-hosted on the same Cloudflare account as the license server, or a separate provider for privacy hygiene?

---

## What Ships in v1 (Definition of Done)

A user in Prague downloads VoiceInk, opens it, and:

1. ✅ The entire UI is in Czech (autodetected from system locale)
2. ✅ Onboarding offers to download Whisper Large (multilingual) or use a cloud key
3. ✅ Settings → Output → Professional Profiles shows 8 Czech profiles
4. ✅ User picks "Lékařská zpráva", binds primary shortcut Alt+Mezerník
5. ✅ User dictates: "Pacientka přichází s positive PCR test, indikujeme follow-up za týden, lab values jsou stable"
6. ✅ Output: "Pacientka přichází s positive PCR test, indikujeme follow-up za týden. Lab values jsou stable." — English terms preserved, Czech grammar corrected, comma added, period normalized
7. ✅ Pasted directly into Word
8. ✅ Crash log shows the language resolution, preserve-list application, and cleanup success
9. ✅ Privacy mode toggle works; with it on, no network traffic during recording
10. ✅ Paddle checkout in Czech, in CZK, with DIČ field for B2B

If all 10 work end-to-end on a clean machine, v1 ships.
