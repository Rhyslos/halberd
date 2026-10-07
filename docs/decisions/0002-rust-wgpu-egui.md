# 0002. Rust, wgpu and egui

- **Status:** Accepted
- **Date:** 2026-10-07

## Problem

Performance and stability are the project's top priorities. The editor must run large maps smoothly and must not crash, and Linux support is a goal (Hammer and Hammer++ are Windows only).

## Options

1. **C++ with Qt and Dear ImGui:** fastest and most mature UI, but crash-prone memory bugs and complex cross-platform builds.
2. **C# with Avalonia:** fast to develop, occasional garbage-collection pauses.
3. **Electron or Tauri with a web UI:** fastest to prototype, but WebGL limits the viewport and memory use is high.
4. **Rust with wgpu and egui:** C++-level speed with memory safety, one-command builds on every platform, one GPU API over Vulkan, DirectX 12 and Metal.

## Decision

Rust for everything, wgpu for rendering, egui (with egui_dock) for panels. Lua (through mlua) later for plugins, because GMod's community already knows it.

## Consequences

- Memory-safety bugs, the classic cause of editor crashes, are ruled out by the compiler in safe code.
- egui is immediate-mode and less polished than Qt for complex property editors. Phase 0 includes building real panels, which tests this early; if it falls short, a new decision record will weigh Qt.
- macOS builds come for free from CI but are untested by the project lead (community-supported tier).
