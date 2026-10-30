# Hub (skeleton, NOT built or verified)

Planned: Svelte + TypeScript UI inside a Tauri 2 shell, talking to `plinth-core` through typed commands. Screens: Scenes, Library, Permission Center (grants, activity log, revoke, Safe mode), Remix editor, Settings.

Intentionally not scaffolded yet: `npm create tauri-app` pulls a large dependency tree and the stack choice is gated on Spike B (WebView2 memory). Create it after Spike B passes:

```bash
pnpm create tauri-app hub --template svelte-ts
```
Then add `plinth-core` as a path dependency of `src-tauri`.
