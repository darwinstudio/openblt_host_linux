# AGENTS.md — openblt_host_linux

## What this is

Tauri 2 desktop app for flashing OpenBLT firmware over RS232. Vue 3 + TypeScript frontend, Rust backend with FFI to a pre-built `libopenblt.so`.

## Commands

- `pnpm install` — install JS deps (pnpm is the package manager; lockfile is `pnpm-lock.yaml`)
- `pnpm dev` — Vite dev server only (port 1420)
- `pnpm build` — typecheck (`vue-tsc --noEmit`) then Vite build
- `cargo tauri dev` — full dev mode (starts Vite + Rust sidecar)
- `cargo tauri build` — produces `.deb` package

No lint, test, or formatter scripts exist. TypeScript is strict (`noUnusedLocals`, `noUnusedParameters`).

## Architecture

- `src/App.vue` — single-file Vue app, all UI logic lives here
- `src/main.ts` — Vue bootstrap
- `src-tauri/src/lib.rs` — Tauri commands (`version`, `program`, `firmware_info`)
- `src-tauri/src/openblt.rs` — unsafe FFI bindings to `libopenblt.so`
- `src-tauri/build.rs` — links `libopenblt.so`, sets rpath for dev and installed paths
- `src-tauri/libopenblt.so` — pre-built shared library (not built from source)

## FFI gotcha

`libopenblt.so` is committed in `src-tauri/`. The `build.rs` copies it to `target/<profile>/` at build time and sets rpath. If you move or rebuild the Rust side, verify the `.so` is still reachable — runtime crashes with "cannot find libopenblt.so" are rpath issues.

## UI framework

Naive UI (`naive-ui`) — all `N*` components come from there. No custom component library.

## Conventions

- Chinese comments and UI text throughout — keep them consistent when editing
- Settings are persisted to `localStorage` (key `openblt.settings`), not files
- Backend events: `progress` (u8 %), `log` (string), `done` (bool)
