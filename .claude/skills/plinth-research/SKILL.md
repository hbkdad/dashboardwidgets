---
name: plinth-research
description: Evidence-first research protocol for the Plinth desktop-widget project. Use when researching competitors, user demand, pricing, or any market claim for Plinth; when asked to "research", "validate", or "check the market" for this project; and before changing the PRD on the basis of new findings.
---

# Plinth research protocol

Goal: findings the owner can trust and act on. Never present a guess as a fact.

## Labels (mandatory, inline)
- **FACT**: read directly from a source you fetched; cite URL and access date in `research/sources.md`.
- **CLAIM**: stated by a party with an interest (a competitor, a vendor). Say whose.
- **INFERENCE**: your judgment. Say what it rests on.
- **UNVERIFIED / NOT TESTED**: searched and not found, or not run.
Weak sources (single blog, SEO page, AI-generated wiki, third-party aggregator) are marked `W` in sources.md and never carry a design decision alone.

## Procedure
1. State the question and what would change a decision.
2. Search in parallel (3-6 queries), then fetch the primary pages (docs, repo, release notes, issue tracker) rather than trusting snippets. If a fetch fails (403/redirect), say so and try an alternative; do not fill the gap from memory.
3. Mine consumer voice, not just feature lists: complaints, requests, workarounds, what people copy or share. Record the *pattern* and the strength of evidence; do not invent quotes (use none unless verbatim and sourced).
4. Check the competitor's current state (versions, release dates). Competitors change: Rainmeter 5.0 and Seelen UI's permission model both appeared mid-project.
5. Write findings to `research/` (new file or an update block at the top of the relevant file), add sources, then list the design consequences.
6. If a finding contradicts earlier docs, edit the earlier docs in the same change and note it in `CHANGELOG.md`.

## Output shape
Question, Findings (labelled), What changes in the product, What is still unknown and how to close it.
