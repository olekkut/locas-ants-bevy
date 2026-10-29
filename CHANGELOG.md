# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses [Semantic Versioning](https://semver.org).

## [0.3.0] - 2026-10-29

### Distribution
- Sprites (`assets/`) and the four built-in maps are compiled into the executable, so a single file runs anywhere.
  A `maps/*.txt` file on disk still overrides the built-in copy.
- Releases publish a Windows `.exe`, a universal macOS `.app` (Apple Silicon and Intel) and a Linux binary, plus SHA-256 checksums.
- Windows release builds open without a console window and use DirectX 12 by default; `WGPU_BACKEND` overrides it.
- The app no longer creates a `maps/` folder at startup; it is only created when saving with `S`.

### Fixed
- Never-written pheromone cells (timestamp 0) no longer attract freshly spawned ants towards the bottom-left map corner.
- A malformed map file is rejected as a whole instead of leaving the grid half-loaded.
- Loading a map clears stale portal links from the previous map and resets a half-placed portal pair.
- The ground texture now tiles reliably: the repeat sampler is set when the image loads, so a race with texture loading can no longer stretch it.
- Ants added with `ants+` (and at startup) spawn at the actual cave center of the loaded map, not at a hard-coded position.
- Clicking the pending blue portal twice no longer leaves an unlinked orange portal.
- Camera panning no longer jumps because of stale mouse-motion events.
- Cells drawn in the editor get the `GridSprite` marker, the same as cells created at startup.
- Cell sprites are only rewritten when their texture or color changes, so Bevy no longer re-extracts every sprite on every frame.

### Changed
- Renamed the crate to `locas-ants-bevy`.
- Removed the once-per-second FPS log line, which reported a hard-coded "100,000 ants". The sidebar shows live FPS and the ant count.
- Moved the duplicated cell-sprite and ant-spawn code into `spawn_cell_sprite` and `new_ant_bundle`.
- Dependencies are optimized in debug builds, so `cargo run` is fast enough to use.

### Added
- Unit tests for pheromone packing, atomic updates, grid indexing and map I/O.
- CI with rustfmt, Clippy and tests on Linux, Windows and macOS, plus a tag-triggered release workflow.
- MIT license with attribution to the original project, a README with screenshots, and a changelog.

## [0.2.2]

- Initial Rust/Bevy ECS port of [piXelicidio/locas-ants](https://github.com/piXelicidio/locas-ants):
  parallel ant update, lock-free pheromone grid, sidebar UI, map editor, portals and textured rendering.
