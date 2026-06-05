# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

First release. Targets `0.1.0`.

### Added
- `ColorEditor` — a render-agnostic color-editing state machine: RGB / HSL / HSV kept in
  sync, keyboard focus and tab order, inline hex and numeric text editing with validation,
  and mouse hit-testing via `focus_for_point`.
- `ColorPicker` — a themable `StatefulWidget` that draws the whole picker, with a
  `PickerTheme` palette and an optional before→after swatch (`.original(...)`).
- `picker_layout` / `PickerRects` — control geometry shared by rendering and hit-testing,
  including per-channel RGB slider bars so the sliders can be mouse-dragged.
- Color helpers: `RgbColor` (hex parse/format, `From`/`Into` for `[u8; 3]` and ratatui
  `Color`), `hsv_field_cell`, `contrast_text`, `split_three`, `srgb_f32`, `normalize_hue`.
- `examples/demo.rs` — a runnable picker with full keyboard and mouse support.

[Unreleased]: https://github.com/allisonhere/ratatui-color-picker
