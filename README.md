# ztrim (Lossy & Lossless Video Trimmer)

A lightning-fast, pure native Rust video trimming and compression desktop tool built with **egui** and hardware-accelerated **NVIDIA NVENC VBR encoding**, plus instant **Lossless Cut stream copying** (`-c copy`).

https://github.com/user-attachments/assets/be507e37-3843-47ca-b13c-3132e88698a4

## Lossless Video Trimming with No Compression
<img width="559" height="241" alt="ztrim_5xWvVOk3xx" src="https://github.com/user-attachments/assets/c9f6838f-8275-4538-93c0-dc93a19949b1" />

## Optional NVENC Video Compression Settings for Smaller File Sizes
<img width="358" height="327" alt="image" src="https://github.com/user-attachments/assets/77819f03-f0e1-433a-b5a9-7173a2021e07" />

---

## Key Features

- **Blazing Fast Native Rust Binary**: Pure Rust UI using `eframe`/`egui` (no Electron, no webview runtime, zero bloat, ~11MB standalone binary).
- **Windows Explorer Right-Click Integration**: One-click install to add `"Trim & Compress with ztrim"` directly to your Windows Explorer context menu for all video files.
- **Interactive Start/End Trim Tabs**:
  - Drag Start (`[`) and End (`]`) tabs directly on the full-width timeline scrub bar.
  - Quick keyboard shortcuts: `I` to set start point, `O` to set end point.
  - Frame stepping (`Left` / `Right` arrows) and Play/Pause (`Space`).
  - Audio playback synchronization via `rodio` with volume slider and mute toggle.
- **Smart Adaptive Action Button**:
  - Automatically switches between **"COMPRESS"** (full video) and **"TRIM & COMPRESS"** when trim points are set.
  - When compression is unticked, switches to **"TRIM (LOSSLESS)"** for instantaneous, 0% quality loss direct stream copying (`-c copy`).
- **NVIDIA NVENC VBR Quality Presets**:
  - **1080p**: 6 Mb/s avg, 25 Mb/s max (NVIDIA NVENC HEVC)
  - **1440p**: 9 Mb/s avg, 30 Mb/s max (NVIDIA NVENC HEVC)
  - **4K**: 14 Mb/s avg, 45 Mb/s max (NVIDIA NVENC HEVC)
  - **Source Res (Custom VBR)**: Keep original resolution.
  - Custom preset creator: save your own bitrate and resolution presets (`+ Save as New Preset...`).
- **Fire-and-Advance Workflow (Zero Waiting)**:
  - When you hit **TRIM & COMPRESS** (or press `Ctrl+Enter`), the render job is dispatched to a background worker queue with real-time speed & progress tracking.
  - The editor **immediately advances to the next untrimmed video in the folder**!
  - **Internal History Log**: Records trimmed videos so sequential navigation never loops over already processed files.
- **In-Place Replacement**:
  - Automatically renders to a temporary file, deletes the original, and moves the compressed cut into its exact place with the original filename.

---

## Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| `Space` | Play / Pause |
| `Left Arrow` | Step -1 frame |
| `Right Arrow` | Step +1 frame |
| `I` | Set Trim Start point |
| `O` | Set Trim End point |
| `Ctrl + Enter` | Fire off job & advance to next untrimmed video |
| `Scroll Wheel` | Zoom canvas into cursor position |
| `Double Click` | Reset zoom & pan (100% fit) |
| `Drag Canvas` | Pan video |

---

## Building from Source

```powershell
cargo build --release
```

The resulting binary will be placed at `target/release/ztrim.exe`.

