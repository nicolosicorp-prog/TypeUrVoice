# Roadmap: Becoming #1 Voice-to-Text App in Central Europe & Poland

**Owner:** Tom
**Target market:** Czechia, Slovakia, Poland (≈70M people, ~25M PC users)
**Wedge:** The only dictation app that genuinely *works* in Czech/Slovak/Polish, fully local optional, on both macOS and Windows
**Position vs. Wispr/Willow/Superwhisper/OpenWhispr:** Niche-first, language-defensible, expand later

---

## Executive Summary

You are NOT going to win a head-on fight against Wispr Flow and Willow in English-speaking markets. They have funding, polished marketing, and YouTube creators on payroll.

You CAN win Central Europe + Poland because:

1. **No competitor markets in Czech/Slovak/Polish.** They all serve English-first; Slavic language quality is an afterthought.
2. **You're already on Windows.** Most Mac-focused competitors arrived on Windows recently and treat it as secondary.
3. **You have BYOK.** Professional users (lawyers, doctors, accountants) in CE/PL are price-sensitive — €15/mo subscriptions feel expensive at local salary levels.
4. **GDPR + "data stays local" is a real, sellable story** to Czech/Polish enterprises and regulated professions.

The plan below assumes you go all-in on this wedge for 6 months, then evaluate expansion.

---

## Phase 0 — Blockers (Week 1)

These prevent literally everything else. Do them first.

### 0.1 Rebrand

The name "VoiceInk" is owned by Beingpax (tryvoiceink.com, ~4.7k GitHub stars, paid product). You will lose every SEO battle and risk a trademark complaint.

**Naming criteria:**
- Pronounceable in Czech, Slovak, Polish, English without modification
- 4–8 letters, ideally 2 syllables
- Avoid Czech-only words (excludes SK/PL market)
- Avoid English words that mean something weird in CZ/PL (e.g. "Flux" → fine, but check)
- `.com` and `.app` available
- No clash with existing app on App Store / Microsoft Store
- Not "Whisper-*" — overcrowded

**Candidates to test (verify availability yourself):**
- Diktly
- Voxa
- Rikni ("say it" in Czech, short, works in SK/PL phonetically)
- Lumio
- Spik
- Mluvi (CZ/SK-friendly but harder for PL)
- Voci

**Acceptance:** Domain bought, GitHub org renamed, app bundle identifier updated (`com.<newname>.app`), notarization certs updated.

### 0.2 Legal Setup (CZ-based seller)

- Confirm business form: OSVČ (živnostenský list) vs. s.r.o. — for software sales above ~€20k/year, s.r.o. is better for liability and reinvestment.
- **EU VAT OSS registration** (One Stop Shop) — you'll sell B2C across EU; OSS lets you pay all EU VAT through Czech finanční úřad. Mandatory above €10k EU-wide B2C sales/year. Register before launch.
- **GDPR data processor agreement template** ready for B2B buyers (CZ law firms will ask).
- Czech terms of service + GDPR notice on the website in CZ, SK, PL, EN.

### 0.3 Payment Infrastructure

- **Paddle** (Merchant of Record) — handles EU VAT, Czech invoicing requirements, refunds, chargebacks. Takes ~5% + $0.50/transaction. Worth it.
- **Alternative:** Lemon Squeezy (similar model, slightly cheaper, less Czech-specific).
- **Do not** use plain Stripe — you'd owe VAT in every EU country separately.
- Support CZK, EUR, PLN pricing display (geo-detected). Charge in EUR universally to simplify accounting.

---

## Phase 1 — Language Excellence (Weeks 2–6)

This is the actual product moat. Without this, you have nothing.

### 1.1 Czech-Specific Transcription Quality

Whisper-Large is decent at Czech but breaks on:
- Proper nouns (Czech names, places)
- Technical/legal/medical vocabulary
- Diacritics on short clips
- Code-switching CZ↔SK or CZ↔EN

**Required improvements:**

- [ ] **Pre-built personal dictionaries** for the top 3 professional segments:
  - **Právo (Law):** ~500 terms — žalobce, obžalovaný, předvolání, exekuce, advokátní kancelář, atd.
  - **Medicína (Medicine):** ~800 terms — diagnózy MKN-10 zkratky, latinské pojmy, léky from SÚKL database
  - **Účetnictví (Accounting):** ~400 terms — DPH, daň z příjmů, faktura, závazek, pohledávka, account names (you already know this from Documiner/CoreBridge)
- [ ] Ship these as **importable .json packs** users can enable in Settings → Dictionary → "Import pack"
- [ ] Same for Slovak (smaller dictionaries — Slovak professional vocab overlaps ~70% with Czech)
- [ ] Polish dictionaries: prawo, medycyna, księgowość

### 1.2 Czech-Aware Cleanup Prompts

Generic "fix grammar" prompts produce *worse* Czech than no cleanup. Build dedicated prompt templates:

- [ ] `clean.cs` — Czech-specific cleanup prompt that knows:
  - vykání/tykání consistency (formal vs. informal you — switching mid-text is the #1 failure mode)
  - Czech comma rules before subordinate clauses
  - i/y rules (vyjmenovaná slova)
  - shoda přísudku s podmětem (subject-verb agreement, gendered past tense)
- [ ] `clean.sk` — Slovak equivalent
- [ ] `clean.pl` — Polish equivalent
- [ ] `polished.cs` — formal Czech ("vážený pane / vážená paní" salutations, proper closings, no anglicisms)
- [ ] Pre-built profiles users can pick: "Email zákazníkovi", "SMS kolegovi", "Soudní podání", "Lékařská zpráva"

**Test rig:** Record 30 short Czech clips with known errors (filler words, missing diacritics, mid-sentence English code-switch). Cleanup output should be measurably better than raw Whisper output by your eye and by an LLM judge.

### 1.3 Localized UI

- [ ] Full Czech UI translation (settings, tooltips, errors, onboarding)
- [ ] Full Slovak UI translation
- [ ] Full Polish UI translation
- [ ] Auto-detect from system locale; manual override in Settings
- [ ] Czech keyboard hotkey hints — show `Alt+Mezerník` not `Alt+Space` on Czech keyboards

### 1.4 Locale-Aware Formatting

After cleanup, output should respect locale:
- [ ] Czech number format: `1 234,56 Kč` (space thousands, comma decimal)
- [ ] Czech date format: `12. 11. 2026` (not `11/12/2026`)
- [ ] Polish: `1 234,56 zł`
- [ ] Detect locale from active app + system locale + clipboard contents

---

## Phase 2 — Product Polish for the Niche (Weeks 5–10, parallel)

### 2.1 Professional Modes (the actual sellable feature)

Ship hardcoded named profiles that solve a specific job:

- [ ] **"Diktát soudního podání"** — formal CZ legal style, paragraph numbering, no fillers, dates in long format
- [ ] **"Lékařská zpráva"** — Czech medical letter style with structured sections (anamnéza, vyšetření, závěr, doporučení)
- [ ] **"Email klientovi"** — formal CZ business letter with proper salutations
- [ ] **"Faktura — popis služby"** — short, formal, fits invoice line items
- [ ] **"Slack zprávy"** — casual, no formal openers, English/Czech mixed
- [ ] **"Code commit message"** — English, imperative mood, under 72 chars first line

These profile names ARE the marketing. A lawyer on a landing page sees "Diktát soudního podání" and instantly understands.

### 2.2 Specific Workflow Integrations

What Czech professionals actually use, and where you should paste directly:

- [ ] **Microsoft Word** (CZ legal/medical world lives here) — verify paste works in 365 + standalone, on Win + Mac
- [ ] **Outlook** (corporate CZ standard) — same
- [ ] **Pohoda/Money S3** ERP (you know these from CoreBridge) — verify paste into notes/popis fields
- [ ] **ISDOC viewer apps** — niche, but cheap to verify
- [ ] **Datová schránka** web UI — every Czech business uses this for legal correspondence

### 2.3 "Privacy Mode" Badge

Build a one-click toggle: **"Plně offline režim"** → disables all cloud LLMs, switches to local Whisper + Ollama, sets a green badge in the menu bar. This is the screenshot you'll use on every landing page section aimed at lawyers/doctors/govt.

### 2.4 Activation & Trial

- [ ] 14-day full-feature trial (longer than competitors' 7 days — matters for slower CZ B2B sales cycles)
- [ ] License key delivered by email immediately on Paddle purchase
- [ ] Offline activation supported (legal sector requirement — many gov't lawyers can't talk to your activation server)
- [ ] 3-device default activation (matches Beingpax's $49 tier — undercut perception)

---

## Phase 3 — Pricing (Week 6)

### Recommended pricing (lifetime, one-time)

| Tier | Price (EUR) | Price (CZK) | Price (PLN) | Devices | Use case |
|---|---|---|---|---|---|
| Personal | €29 | 749 Kč | 129 zł | 1 | Solo user |
| Pro | €49 | 1 249 Kč | 219 zł | 3 | Most buyers |
| Team | €99 | 2 499 Kč | 439 zł | 10 | Small office/law firm |

- **No subscription tier for v1.** Subscriptions hurt BYOK products and slow CE B2B sales (they prefer capex).
- **14-day money-back guarantee** — required for trust at this price.
- **Pricing page in 4 languages**, prices in local currency by geo-IP.
- **Czech invoice with DIČ** (VAT ID) automatic on B2B — Paddle handles this; verify it works for CZ buyers specifically before launch.

### Upsell: Pro Dictionary Packs (Phase 5+)

- Sell curated, maintained professional dictionaries as €19 add-ons after launch:
  - "Právník Pack" — 5 000+ legal terms, judge names, court abbreviations
  - "Lékař Pack" — MKN-10 codes, ATC codes, drug names from SÚKL
  - "Účetní Pack" — full chart of accounts, DPH terminology
- Margin is ~95%, and they reinforce the niche position.

---

## Phase 4 — Distribution & Marketing (Weeks 7–14)

### 4.1 Landing Page

- [ ] **Domain:** `<newname>.app` primary, `.cz` redirect
- [ ] **Hero:** 15-second screen recording showing Czech dictation → polished email in Outlook
- [ ] **Below fold:** 3 named professional profiles with example before/after
- [ ] **Privacy section:** "Vaše data zůstávají u vás" with the offline-mode screenshot
- [ ] **Pricing section** in local currency
- [ ] **Comparison table** vs Wispr/Willow on Czech/Slovak/Polish quality (you'll dominate)
- [ ] FAQ in 4 languages
- [ ] **No signup wall** — direct DMG/MSI download for trial

### 4.2 Czech Tech Press (in order of impact)

- [ ] **Lupa.cz** — pitch as "Český vývojář vydává konkurenci k Wispr Flow"; angle: local, privacy, BYOK. Email editor directly, don't use PR forms.
- [ ] **Živě.cz** — gadget/software review; offer free Pro keys for the staff.
- [ ] **Root.cz** — technical readers, focus on the offline mode, Whisper, Ollama integration.
- [ ] **Connect.cz** — mainstream tech, more lifestyle angle.
- [ ] **Cnews.cz** — review angle.

### 4.3 Slovak Press

- [ ] **Živé.sk** — biggest SK tech site
- [ ] **Touchit.sk** — gadget reviews
- [ ] **DSL.sk** — technical readers

### 4.4 Polish Press

- [ ] **Antyweb.pl** — broad tech, good reach
- [ ] **Spider's Web** — modern tech site
- [ ] **Niebezpiecznik.pl** — privacy/security angle; the offline mode story is THEIR story
- [ ] **Komputer Świat / dobreprogramy.pl** — software-specific

### 4.5 Communities

- [ ] **r/Czech**, **r/Polska**, **r/Slovakia** — soft launch posts ("I built this, here's what I learned")
- [ ] **r/MacApps**, **r/macOS**, **r/Windows10** — English launch, mention CZ angle as differentiator
- [ ] **HackerNews "Show HN"** — once polished; the Tauri 2 + Rust + Whisper.cpp + multi-LLM story is HN-ready
- [ ] **Facebook groups:** "Mac uživatelé ČR/SK", "Apple Czech", "Polski Mac"
- [ ] **dev.cz forum** + **devel.cz** Slack — Czech dev community
- [ ] **LinkedIn:** post in Czech/Polish for B2B reach, target lawyers, accountants, doctors

### 4.6 YouTube / Content

- [ ] Czech YouTubers: contact 5–10 productivity/tech channels (e.g. Honza Komínek, Cnews) — offer free Pro license + script suggestions
- [ ] Polish: contact 5 productivity YouTubers
- [ ] Your own short demo videos: "Diktování v češtině do Wordu", "Lékařská zpráva za 60 sekund" — these double as ads and as evidence on the landing page

### 4.7 Direct B2B Outreach (the actual revenue)

After 100 paying individuals, switch focus:

- [ ] Email 50 Czech advokátní kanceláře (law firms) — offer 30-day Pro trial for the firm
- [ ] Email 30 lékařské praxe (medical practices) — same offer
- [ ] LinkedIn DM 50 účetní firmy (accounting firms)
- [ ] Czech bar association (ČAK), medical chamber (ČLK), chamber of tax advisors — offer member discount

This is where the real money is. One 20-lawyer firm = €1 000.

---

## Phase 5 — Launch Sequence (Weeks 12–16)

### Week 12: Soft Launch
- Ship to your own Czech network (50 people, free Pro keys for early feedback)
- Fix the bugs they find — there will be many

### Week 13: Czech Press Embargo
- Reach out to Lupa.cz + Živě.cz + Root.cz with embargoed review copies
- Coordinate publication date

### Week 14: Public Launch
- Press goes live
- Reddit posts (r/Czech, r/MacApps)
- LinkedIn announcement in CZ + EN
- Twitter/X if you use it

### Week 15: Slovak + Polish Launch
- Adapted landing page in SK + PL
- Slovak and Polish press outreach
- Reddit r/Polska, r/Slovakia

### Week 16: Hacker News + Product Hunt
- "Show HN: Voice dictation for Central European languages that actually works"
- Product Hunt launch — coordinate with someone who has a list to hunt you

**Goal at end of week 16:** 200 paying customers (~€8 000–10 000 gross). If you hit this, the wedge works.

---

## Phase 6 — Expansion Plan (Months 4–6)

Only after CE/PL works:

- [ ] **Hungary** (~10M, no serious local dictation player, similar professional profile)
- [ ] **Romania** (~19M)
- [ ] **Croatia + Slovenia** (small but easy to add after PL/HU)
- [ ] **DACH** (Germany/Austria/Switzerland) — much harder, but German pro tools are surprisingly bad at modern AI dictation; possible expansion
- [ ] **English markets** — only after you have ~1 000 paying CE customers; at that point you have testimonials, revenue, and a real story

---

## Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Wispr ships a "Czech mode" upgrade and undercuts your differentiation | Medium | Move fast, lock in B2B contracts before they react |
| OpenWhispr adds Czech dictionaries community-contributed | Medium | Your dictionaries should be curated/professional quality, not generic word lists |
| Microsoft Voice Typing becomes good enough in Czech | Low (2–3 yrs) | Free baseline always exists; sell on workflow, profiles, professional features |
| Apple ships better dictation in macOS 16+ | High | You have Windows + Linux future + better cleanup; Mac becomes one of three platforms |
| Slow B2B sales cycle in CZ (4–6 months from interest to purchase) | High | Combine with B2C volume revenue; don't bet only on enterprise |
| Whisper local quality plateaus | Low | Add Parakeet (NVIDIA) support like OpenWhispr did — gives you Windows GPU acceleration story |

---

## Success Metrics

### Month 3 post-launch
- 200 paying customers
- ~€8 000 gross revenue
- 1 published Czech review (Lupa or Živě)
- Refund rate <5%

### Month 6 post-launch
- 1 000 paying customers
- ~€40 000 gross revenue
- 3+ Czech B2B contracts (law/medical firms, 5+ seats each)
- Active in CZ, SK, PL with localized landing pages
- One viral moment (HN front page, or Polish YouTuber video)

### Month 12 post-launch
- 3 000+ paying customers
- ~€120 000 gross revenue
- Recognized name in CE pro market
- Optional: hire 1 part-time person for support/sales
- Decide: expand to DACH, or go deep on B2B in CE

---

## What NOT to Do

- ❌ Don't try to compete with Wispr on English-speaking marketing in year 1.
- ❌ Don't launch a subscription tier — BYOK + one-time fits the market and your model.
- ❌ Don't go on Mac App Store yet — 30% cut + sandbox fights your hotkey/paste flow.
- ❌ Don't pitch as "another Whisper wrapper" — pitch as "the dictation app for Czech/Polish professionals."
- ❌ Don't build features that don't directly serve a CE/PL professional buyer until you hit €40 000 ARR.
- ❌ Don't open-source the app — Beingpax did this and now has competitors using their code. You can be source-available later if useful, but not GPL.
- ❌ Don't promise enterprise features (SSO, SCIM, on-prem) until a real customer pays for them upfront.

---

## Open Decisions (need Tom's input)

1. **Final name** — pick from Phase 0 candidates or generate new ones.
2. **Business entity** — OSVČ for fast launch or set up s.r.o. now for proper structure?
3. **Personal time commitment** — full-time for 6 months, or part-time alongside Documiner/ModelX/CoreBridge?
4. **Initial marketing budget** — €0 (organic only), €2k (some YouTube placements), or €10k (real ad spend on Czech LinkedIn)?
5. **Open-source decision** — closed-source paid (recommended), source-available, or fully OSS with paid binaries?

---

*Last updated: 2026-05-11*
