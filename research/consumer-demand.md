# Consumer demand research (accessed 2026-10-30)

Question: what do people actually want from desktop customization, and what should change in Plinth because of it? Evidence is qualitative: tech press, guides, forum threads, and community write-ups. **No survey data and no verbatim user quotes were obtained; nothing below is a measured market size.** Labels as in `.claude/skills/plinth-research`.

## Findings
| # | Pattern | Evidence | Strength |
|---|---|---|---|
| D1 | **Widgets directly on the desktop** is the most-cited wish; Microsoft's Widgets board is a flyout, not a desktop surface, and users go to third-party tools to get desktop placement | Tech-press reviews of the Widgets board and 2025-2026 roundups of "desktop widgets Microsoft won't give you" (XDA, Windows Latest, Yahoo/ZDNet-syndicated pieces) | FACT that these articles say it; demand size UNVERIFIED |
| D2 | People want a **useful dashboard** (calendar, tasks, weather, system stats), resizable, without a news feed | Same sources; complaints: limited customization, too much news, not all widgets resizable, "only a few useful" of the 56 Store widgets | Moderate (press) |
| D3 | **Setups get shared and copied.** In ricing culture people post screenshots, others copy them, and sharing the config files (dotfiles) is normal; motivation includes self-expression, control, tinkering, workflow fit, and community status | Linux ricing guides and community write-ups (HackCurio, TypeCraft, venam, DEV posts) | Moderate; Linux-centric, transfer to Windows is INFERENCE |
| D4 | **Cost anxiety**: Wallpaper Engine users report battery drain, frame drops with the UI open during games, heavy items using lots of CPU, and want frame-rate caps that are respected and a built-in CPU/RAM monitor | Steam community threads, MiniTool FAQ (forum anecdotes) | Moderate (anecdotal) |
| D5 | **Discovery is cluttered**: low-effort Workshop submissions bury good work; search is weak | checkthat.ai/VaporLens summaries (automated, weak) and one Steam thread | Weak |
| D6 | **Static rules disappoint**: in one study, 7 of 8 participants with productivity-tool experience were unhappy with rule-based, context-blind systems; focus modes exist in Windows/macOS already | arXiv paper on intentional digital living and human-factors write-ups | Weak-moderate (small sample) |
| D7 | Rainmeter remains "the most powerful" but needs effort; easier alternatives are what beginners are pointed to | SaaSHub/AlternativeTo comparisons | Weak (directories) |
| D8 | Overall interest is in "setup culture": r/unixporn ~641k members and growing, r/Windows11 ~281k, r/desktops ~141k; Rainmeter ~322k flat (third-party counts, undated) | GummySearch | Weak (undated) |

## What changes in Plinth (decisions)
1. **Setup Cards become a headline feature, not a side feature** (D3). A card describes a whole desktop (Scene + layout + accent), contains no code, adapts to any monitor, shows combined permissions up front, offers a 60-second try-out with undo, and carries a remix credit chain so creators are recognized when others build on their work. This is the "dotfiles for normal people" idea made safe and one click. Code: `plinth-core::setup` (parse/validate/summary/fit), prototype "Share setup".
2. **Impact Meter** on every Scene and widget (D4): estimated RAM, CPU, and "pauses in games", replaced by live counters as soon as the host reports them. Honest, our own measured numbers (`plinth-core::impact`, spikes). It also turns our measured isolation cost into a feature: slots meter in Safety.
3. **Moods** (D6): context rules that switch Scenes (fullscreen, battery, time, foreground app) with three guardrails the research supports: never override a manual choice in the same situation, minimum dwell time, and **suggest, don't auto-create** rules after repeated manual switches. Code: `plinth-core::context` (Engine, Learner), prototype "Moods".
4. **Edit layout on the real desktop** (D1, D2): drag and keyboard-move widgets with grid snap directly on the live preview; resizable, no news feed, no ads. Prototype "Edit layout"; builder work continues in v1.0.
5. **Curation over volume** (D5): store listings need a measured Impact badge and review; staff picks first. No open "upload anything" feed.
6. Safety stays the trust foundation but is no longer the only headline: the pitch becomes **"Make it yours, share it in one click, keep it cheap on your PC, and know it is safe."**

## Still unknown (how to close)
Real willingness to pay; whether Windows users share setups the way Linux users do (test with a waitlist page and a "post your setup card" campaign); streamer overlay demand (not covered by sources found); real user quotes (do the 15 interviews in the roadmap). Do not market these as proven.
