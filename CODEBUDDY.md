# CODEBUDDY.md

This file provides guidance to CodeBuddy Code when working with code in this repository.

## Overview

A **Tauri 2 desktop GUI** for flashing firmware onto microcontrollers with [OpenBLT](https://www.feaser.com/openblt/). It is a thin front-end over the OpenBLT host C library (`libopenblt.so`):

- **Frontend**: Vue 3 + TypeScript (Vite), UI built with Naive UI. Lives in `src/`.
- **Backend**: Rust (Tauri command handlers) in `src-tauri/src/`. It loads `libopenblt.so` and drives the flashing flow through hand-written `#[repr(C)]` FFI bindings (`src-tauri/src/openblt.rs`).
- **The C library itself is NOT built here.** `src-tauri/libopenblt.so` is a checked-in binary copied from the sibling `Host/` directory (built from `Source/LibOpenBLT`). See "Gotchas" for the sync requirement.

There is **no test suite and no dedicated linter** in this repo. Correctness is validated by running the app against hardware.

## Commands

Package manager is **pnpm** (see `pnpm-lock.yaml`). `node_modules/.bin` provides `vite`, `vue-tsc`, and `tauri`.

```bash
# Install JS deps
pnpm install

# Frontend dev server only (Vite, port 1420) — no Rust rebuild
pnpm dev

# Type-check + production frontend build -> dist/  (this IS the lint/typecheck step)
pnpm build

# Type-check only, no emit:
pnpm exec vue-tsc --noEmit

# Run the full desktop app (builds Rust backend + frontend, hot-reload)
pnpm tauri dev

# Build the distribution bundle (appimage/deb/etc. per tauri.conf.json "bundle")
pnpm tauri build

# Rust-side type/lint check without a full Tauri build:
cd src-tauri && cargo check

# Preview the built frontend (no Rust):
pnpm preview
```

There is no "run a single test" command — the project has no tests.

## Architecture (the big picture)

```
src/App.vue  (Vue UI: transport selector, file picker, progress bar, log view)
   │  invoke("program" | "version")          listen("progress" | "log")
   ▼
src-tauri/src/lib.rs   (#[tauri::command] handlers; `program` spawns a std::thread)
   │  calls FFI functions 1:1 from openblt.h
   ▼
src-tauri/src/openblt.rs  (hand-written #[repr(C)] bindings to libopenblt.so)
   │  dl-link at build time (build.rs: link-search src-tauri, link-lib openblt)
   ▼
libopenblt.so   (OpenBLT host library; fields documented in parent Host/CODEBUDDY.md)
```

### Frontend ↔ backend contract
- Frontend calls backend via `invoke("command_name", {args})` (`@tauri-apps/api/core`).
- Backend pushes progress/diagnostics to the frontend via Tauri **events** `emit("progress", u8)` and `emit("log", String)`, consumed with `listen(...)` in `App.vue`. This is how the long flash operation streams UI updates.
- Commands registered in `lib.rs` `run()`: `greet` (scaffold leftover), `version` (returns `BltVersionGetString()` to confirm the FFI/lib are wired), `program`.
- UI exposes only **rs232** and **usb** transports, even though `openblt.rs` also declares CAN/NET/MB-RTU bindings and settings structs.

### The program flow (`run_program` in `lib.rs`)
Mirrors the standard LibOpenBLT call sequence: `BltFirmwareInit(SRECORD)` → `BltFirmwareLoadFromFile` → `BltSessionInit` → `BltSessionStart` → per segment `BltSessionClearMemory` then 256-byte `BltSessionWriteData` chunks (matching the XCP programming block size) → `BltSessionStop`/`Terminate`/`BltFirmwareTerminate`, emitting `progress` (0–100) after each chunk and `log` lines on each event. `program` runs it on a spawned `std::thread` so the Tauri event loop is not blocked.

### FFI bindings (`openblt.rs`) — the fragile part
- The `Blt*Session*`, `Blt*Transport*` settings structs are hand-written `#[repr(C)]` and must match `Source/LibOpenBLT/openblt.h` **field-for-field** (order, type, padding). Type map: `uint32_t→u32`, `uint16_t→u16`, `uint8_t→u8`, `char const*→*const c_char`, `void const*→*const c_void`.
- The **USB** transport has *no* settings struct in `openblt.h` (fixed VID/PID `0x1D50`/`0x60AC`), so `BltSessionInit` is called with a null transport-settings pointer for USB. The `#[repr(C)]` structs for CAN/NET/MB-RTU exist in `openblt.rs` but are unused by the current UI.
- FFI calls are `unsafe`; all wrappers live behind `unsafe` blocks in `lib.rs`. Only `version_string()`/`version_number()` are exposed as safe helpers.

### Linking & runtime .so resolution (`src-tauri/build.rs`)
- At build time, `build.rs` adds `src-tauri/` to the linker search path and links `-lopenblt`, and copies `libopenblt.so` next to the target binary.
- rpath is set to `$ORIGIN` (binary's own dir) then to the absolute `src-tauri/` dir, so the binary finds the `.so` both in `target/<profile>/` and in the source tree.

## Gotchas

- **Keep `src-tauri/libopenblt.so` in sync with the real library.** It is a checked-in copy. If you rebuild LibOpenBLT in the parent `Host/Source/LibOpenBLT` tree (output lands in `Host/libopenblt.so`), copy it here or the app will run against a stale ABI. ABI/struct changes in `openblt.h` require updating the `#[repr(C)]` structs in `openblt.rs` to match, or the app will misbehave/crash.
- **CString lifetimes**: `port_name` and the firmware path are passed as raw pointers into the C library and are read during `BltSessionStart`. The `CString` values (`c_port`, `c_file`) must outlive the whole session — they are kept alive in `run_program`'s scope on purpose. Never pass a temporary.
- **Transport settings must stay alive too**: `rs232_settings` is declared in `run_program`'s scope (not inside the `if`) so its pointer is valid when `BltSessionInit` reads it.
- USB needs no settings and ignores baud/port; the UI shows a fixed VID/PID note for it.
- `tauri.conf.json` uses `csp: null` and a fixed dev port `1420` (`strictPort`). Vite is configured to ignore `src-tauri/**` in its watcher.
- Type-checking the frontend requires `vue-tsc` (`pnpm build`); plain `vite build` would skip type errors. `tsconfig.json` enables `strict`, `noUnusedLocals`, and `noUnusedParameters`.
