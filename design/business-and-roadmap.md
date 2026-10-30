# Business, Go-to-Market and Roadmap: Plinth

All market numbers below are **targets or assumptions**, not forecasts. Price anchors come from `research/market-and-gaps.md` (Wallpaper Engine CDN$6.49 [Steam]; Stardock $9.99/app, ~$39.99 suite per dated sources, UNVERIFIED current).

## 0. Positioning note (post Seelen UI audit)
Seelen UI is free and already has a catalog and a paid tier ("Seelen Sky": sync plus exclusive resources). Plinth therefore cannot win on price or catalog size at launch. Message: "widgets you can trust, built without code, creators get paid." Seeding must target creators Seelen/Rainmeter don't pay. Do not call competitors unsafe in marketing until the hands-on test confirms it; say what Plinth guarantees instead.

## 1. Pricing recommendation
**Free core + paid content marketplace (revenue share) + optional one-time "Plus" license.**
- **Free forever:** the engine, hub, first-party widget set, performance governor, accessibility, revert/profile export, local Scenes, installing free community content, and security updates. Rationale: the free incumbents (Rainmeter, Lively) set the floor; safety/performance must never be paywalled.
- **Plinth Plus (one-time, proposed ~$9.99, untested):** visual builder export of unlimited widgets, animated-wallpaper pack, multi-monitor per-screen Scenes, priority store features. Same price band users already accept (Wallpaper Engine, Fences). **Never disables the app if you stop paying**; perpetual for the purchased major version.
- **Store cut:** creators keep 80%, Plinth 20% before payment fees (assumption; Steam's cut is higher: UNVERIFIED exact; test). 
- Rejected: subscription that locks software (user pain, [Stardock FAQ]), ads, selling user data.

## 2. Creator economics
- Free listings forever; paid listings $1–$15 set by creator; bundles; "pay what you want" tips.
- Why publish here rather than DeviantArt/Steam (INFERENCE, to validate): built-in discovery inside the app, one-click install, update delivery, anti-piracy via signed per-user license, analytics, builder ecosystem, and a real payout path (the category has none found in research). Cost: 20% cut and review friction.
- Anti-ripoff: publisher verification, DMCA workflow, license-bound packages.
- Payments flag: **requires Stripe Connect or similar, KYC, tax handling (VAT/sales tax), and refund policy**: not solo-trivial. Alternative: use Gumroad/Lemon Squeezy as merchant of record for v1.0 to avoid tax liability (fee impact UNVERIFIED).

## 3. Go-to-market
First 1,000 users (specific channels; need community rules checks before posting):
1. **r/Rainmeter, r/desktops, r/Windows11, r/unixporn-adjacent Windows setup posts**: show-don't-tell clips of the 10-minute onboarding; reply to "how do I make my desktop look X" threads with helpful Scenes, not ads.
2. **Rainmeter forum and Discord** (respectfully: "migrate your widgets" tooling later).
3. **YouTube/TikTok desktop-setup creators**: give 20 creators early access plus their own Scene in the store with revenue share (outreach list to be built; reach figures UNVERIFIED).
4. **Product Hunt + Hacker News** "Show HN" focused on the security/performance angle.
5. **Microsoft Store listing** (free tier) and winget package for discoverability.
6. **Creator seeding:** recruit ~20 DeviantArt/Rainmeter skin artists and pay them upfront ($100–$300 each, budget assumption) for exclusive launch Scenes. Budget flag: this is not zero-cost.

**Day-one library (minimum):** 12 Scenes (3 dark minimal, 2 gaming, 2 productivity, 2 streamer, 2 light/pastel, 1 high-contrast accessible), 10 first-party widgets, 30 wallpapers (CC0/commissioned), 3 starter builder templates (v1.0).

## 4. Risks and mitigations
| # | Risk | Type | Mitigation |
|---|---|---|---|
| 1 | Windows update breaks window-layer integration | Technical/platform | Abstraction + fallbacks, Insider CI, remote kill-switch (arch §3, §8) |
| 2 | Marketplace cold-start: no creators or buyers | Competitive | Paid seeding, Rainmeter migration tools, free listings first, launch with first-party content |
| 3 | Malware in store | Security/legal | Permission sandbox, signing, reviews, no executables (arch §7) |
| 4 | Microsoft ships native desktop widgets; Rainmeter 5.0 (shipped Oct 12, 2026) and Seelen UI (free, open-source, same stack) erode the "modern free widgets" pitch | Competitive/platform | Differentiate on sandbox/permissions, curated store with payouts, update-resilient non-shell design; audit Seelen UI's store/permissions in Phase 0 and reconsider (partner/contribute) if it already covers them |
| 5 | Legal: trademark conflict on name, copyright in wallpapers/themes, payments tax | Legal | Clearance search before launch, CC0/commissioned assets only, merchant-of-record provider |
| 6 | Solo-dev burnout / bus factor (Samurize/ObjectDock pattern, INFERENCE) | Operational | Open-source the engine core (decision #2), public roadmap, small scope |
| 7 | WebView2 memory too high vs. "lightweight" promise | Technical | Week-1 prototype gate, declarative-first, Rust renderer fallback |

## 5. Roadmap and success metrics (realistic targets, not hype)
| Phase | Scope | Intentionally excluded | Success metrics (targets) |
|---|---|---|---|
| **0: Validation (4–6 wks)** | Interviews, landing page, perf prototype (WebView2 RAM, WorkerW on current Win11), **hands-on Seelen UI test in a VM (hostile widget, RAM/CPU idle and in-game, Win+D behavior, capability files)**, hostile-widget sandbox prototype | Any marketplace | 15 interviews; ≥300 waitlist emails; prototype passes perf gates or stack switched; Seelen claims confirmed or wedge revised; hostile-widget suite shows 0 escapes |
| **MVP (≈4–6 months solo, INFERENCE)** | M1–M8, 6 Scenes, local gallery, signed first-party content, Microsoft Store/winget | Store, builder, paid features | 1,000 installs; D30 retention ≥25%; idle CPU <0.5%; 0 critical crash/security issues; ≤3 Windows-update breakages unresolved >7 days |
| **v1.0 (+4–6 months)** | V1–V7: builder, store with 10–30 creators, Plus license, animated wallpapers, auto-updates | Cloud sync, AI, other OSes | 10,000 installs; ≥50 store listings; ≥100 paid transactions; Plus conversion ≥2% (assumption); 0 malware listings live >24h |
| **v1.5 (+6 months)** | Cloud sync, Rainmeter import helper (if feasible), AI layout helper, creator analytics | macOS/Linux | 30,000 installs; creators earning ≥$2k aggregate/month (stretch) |

Success numbers are my planning assumptions, not benchmarks from the market; recalibrate after Phase 0.
