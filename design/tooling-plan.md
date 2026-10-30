# Tooling plan: which of your skills, connectors and plugins apply

Decision rule: use a tool only when it improves a deliverable of this project. Anything that sends, publishes, or spends on your behalf is **not** used without your say-so. Reviewed 2026-10-30 against the lists available in this workspace.

## Apply now
| Phase | Tool | Use |
|---|---|---|
| Hub UI design | `newage-webui` (your skill) | Premium dark UI, design tokens, motion, scroll/3D restraint rules, QA gates for the hub prototype |
| Hub UI design | `design:design-system`, `design:design-critique`, `design:accessibility-review`, `design:ux-copy`, `design:design-handoff` | Token system, critique of the prototype, WCAG pass (reduced motion, contrast, scaling are product promises), microcopy for permission prompts, handoff spec for the real Svelte build |
| Engineering | `engineering:architecture`, `engineering:system-design`, `engineering:testing-strategy`, `engineering:code-review`, `engineering:documentation` | ADR for hybrid delivery, test pyramid for the host, review before each merge, docs |
| Product | `product-management:write-spec`, `product-management:roadmap-update`, `product-management:synthesize-research` | Turn PRD into build specs, keep roadmap in sync, synthesize the 15 planned interviews when they exist |
| Dataviz | `dataviz` | Any chart in the Permission Center (activity timeline, memory per widget) |
| Browser | built-in browser (`built-in-browser` skill) | Render and screenshot the prototype to check layout at phone/desktop widths and dark/light |

## Apply later (needs the product to exist or a decision from you)
| Tool | Use | Why later |
|---|---|---|
| `figma` skills and the Figma connector | Component library and flows in Figma, then `figma-to-code` | Creates files in your Figma account; do after the HTML prototype settles the direction |
| Supabase connector | Store backend (listings, licenses, creator payouts metadata) | v1.0; schema design only until a payments decision is made |
| Vercel connector | Host a public landing page/waitlist | Publishing is outward-facing; needs your go-ahead and a domain/name decision |
| Shopify / Stripe-like commerce tools | Merchant-of-record evaluation for creator payouts | Payment decision pending (README decision 2) |
| `marketing:*`, `small-business:*` (seo, content, social, outreach) | Launch content, waitlist copy, creator outreach | After name/trademark clearance |
| `anthropic-skills:docs` / Google Workspace / Notion | Shareable copies of the PRD for creators/advisors | Only if you want it shared outside GitHub |
| `schedule` / `loop` | Nightly Windows-Insider smoke-test reminder | After CI for the layer probe exists |
| `mcp-builder` | An MCP server exposing the Permission Center log to AI tools | Optional, post-MVP idea |

## Not applicable (checked, intentionally unused)
Your trades/ForgeQuote/SENTRY skills (`saas-feature-builder`, `trades-content-writer`, `client-proposal`), Apollo/HubSpot/Twilio/Gmail/Slack/Calendar connectors, payroll/tax/inventory small-business skills, Gamma/pptx/xlsx/docx/pdf (no deliverable of that type requested), Wix/Sanity, Valtown. `digital-product-launcher` (Gumroad/Etsy) could sell widget packs later but conflicts with the store plan; revisit at v1.0.

## Plugins to consider installing (not installed, none needed yet)
A code-signing/CI helper and a Windows-focused test-automation plugin would help once the installer exists. I will search the plugin catalog when that phase starts rather than now.
