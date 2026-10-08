# 0012. The Console and the log file

- **Status:** Accepted
- **Date:** 2026-10-08

## Problem

The Console panel was a plain list of text lines: everything looked the same, nothing told a problem apart from a routine message, and it grew without limit. Problems reported by the libraries Halberd is built on (graphics drivers through wgpu, the window system through winit) went nowhere at all. And when Halberd closed or crashed, everything in the Console was gone, so a bug report could not include it.

## Options

1. **Console only:** give messages levels and filters, but keep them in memory. Simple, but nothing survives a crash, which is when it matters most.
2. **Console plus a log file,** with library messages brought in through Rust's standard logging interface (the `log` library, which wgpu, winit and egui already report through).
3. **A full logging framework** (`tracing` and friends): structured fields, spans, filters by configuration. More than Halberd needs; harder to keep plain.

## Decision

Option 2.

- **Messages have a time and a level:** information, warning or error. Warnings and errors are coloured, and the panel can show or hide each level, search, copy what is shown (for bug reports) and clear itself.
- **Repeats collapse:** the same message straight after itself is shown once with a count (×12), so a noisy message cannot bury the rest.
- **The Console keeps the last 5,000 messages.** Older ones are dropped, with a note; the log file keeps them. Each message takes one line (a longer one is cut short, and hovering shows all of it), and only the lines on screen are drawn, so a full Console costs no more than an empty one.
- **A hidden Console still gets noticed:** when warnings or errors arrive while the Console is behind another tab, the tab shows how many.
- **Library messages:** the `log` interface is connected to the Console. Halberd's own crates report information and up; other libraries only warnings and errors, which are named after the library they came from (for example `wgpu_hal: …`). The logger never calls back into egui or the window, because libraries (egui itself) log while holding their own locks and calling back would freeze Halberd. A separate small thread wakes the window instead, at most four times a second, so a message repeated every frame cannot keep Halberd drawing nonstop.
- **The log file** is `halberd.log`, next to the settings file. Every message goes in it, line by line straight to disk so it survives a crash, and the reason for a crash (a panic) is written in it before Halberd stops. Each run starts a new file; the previous run's is kept as `halberd.previous.log` (if the old file can't be set aside, the new run is added to its end instead of erasing it). One run writes at most 10 MB, and one message at most 64 KB; the crash reason is written even when the file is full. The startup report goes in the file before the window opens, so it is there even if the window cannot open.
- **Times are the computer's local time**, through the `chrono` library (MIT or Apache 2.0).

## Consequences

- A user reporting a bug can attach `halberd.log` (or `halberd.previous.log`, after a crash), or press Copy in the Console.
- Library warnings users cannot act on may show up (for example winit's notes about unusual Linux displays). If one turns out to be noise everywhere, it can be filtered out by name.
- Two copies of Halberd running at once share the log file: the second sets the first one's log aside as `halberd.previous.log`, and both keep writing to their own file.
- Messages from the logger reach the Console on the next frame, so they can appear slightly out of time order with the editor's own messages; each keeps its own time.
