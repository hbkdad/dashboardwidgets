# Naming: first web check of candidate names (2026-10-30)

Purpose: "Plinth" is a codename with real collisions (decisions D2). This is a **first web search only**, not a trademark clearance. A formal search (USPTO, EUIPO, WIPO, plus domain and app-store checks) is still required before a name is used publicly. Labels: FACT (a search result said so), UNVERIFIED (not checked).

| Candidate | What the web search found | Verdict |
|---|---|---|
| Tessera | A Windows desktop gadget app of that name is described as "living gadgets ... translucent glass tiles" (one result is on an unrelated, suspicious domain, so treat it carefully) | Taken in this exact space: avoid |
| DeskForge | A Windows desktop manager with drawers and live widgets, on Steam (release status listed inconsistently: September 2026 and Early Access), with paid add-on packs | Taken in this exact space: avoid |
| Rigdeck / Rigdesk | No relevant results for either spelling. Related products seen: RIGStats (a free hardware monitor for a second display), DeskDash | No hits found; plausible. Still needs a formal search |
| Casemark | No relevant results | No hits found; but "case" suggests PC cases only, and "mark" may read as a trademark word |
| Tessel | No Windows app found by this name; UNVERIFIED whether it clashes with a hardware/dev-board project of the same name (not searched) | Needs a formal search and a check of non-software uses |
| Plinth (current) | FreedomBox Plinth, Plinth UI, an iOS app, an Australian trademark filing for software (earlier research) | Do not use publicly |

Other products seen in the same search (names to stay away from): DeskLayer (Steam; taskbar replacement with live wallpapers and widgets across monitors, not yet released at the time of the listing), BlendWidget, WidgetSpark, Mosaic, Seelen UI, Lively, Rainmeter, DesktopX.

## Recommendation
Shortlist **Rigdeck** and **Tessel** for a formal clearance, with **Casemark** as a backup. Rigdeck fits the PC-builder audience (D16) and had zero hits; Tessel is short and neutral. Do not change code or the repo name until one is cleared. Cost to rename later is small: the name appears in the window title, tray tooltip, folder names under `%APPDATA%` and `%LOCALAPPDATA%`, the single-instance mutex, and the docs.

## Availability checks (2026-10-30, DNS and package registries; FACT as observed, not a registrar check)
| Name | .com | .io | .app | .dev | .net | crates.io | npm |
|---|---|---|---|---|---|---|---|
| Rigdeck | resolves (registered) | resolves | resolves | no DNS record | no DNS record | free | free |
| Tessel | registered | registered | registered | registered | registered | taken | taken |

"No DNS record" does not prove a domain is for sale: a registered domain without DNS also looks like this. Check a registrar before relying on it. The three Rigdeck domains that resolve may be parked or in use (not visited).

## Decision recommended
**Rigdeck** is the better candidate: no software hits, free package names, and `.dev`/`.net` possibly available; Tessel is taken almost everywhere. Next steps before any rename: (1) a formal trademark search for Rigdeck in the US, EU and Australia in the software and online-services classes, (2) confirm `rigdeck.dev` (or another) at a registrar, (3) check what the three resolving domains are. Until then the repo, window title and docs keep the codename.
