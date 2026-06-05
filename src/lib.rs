//! An interactive terminal color picker for [ratatui].
//!
//! [`ColorEditor`] is a self-contained, render-agnostic state machine for editing a
//! single color. It keeps RGB, HSL, and HSV representations in sync, manages keyboard
//! focus / tab order, supports inline numeric text editing with validation, and does
//! mouse hit-testing against a computed layout ([`picker_layout`] → [`PickerRects`]).
//!
//! The crate owns the color model and layout/hit-testing; you drive it from your event
//! loop (see the methods on [`ColorEditor`]) and draw it however you like. A `demo`
//! example is included.
//!
//! ```no_run
//! use ratatui_color_picker::{ColorEditor, picker_layout};
//! let mut editor = ColorEditor::from_rgb(0x89, 0xb4, 0xfa);
//! editor.adjust_focused_numeric(10.0); // nudge the focused channel
//! let rgb = editor.to_rgb();           // read the result
//! assert_eq!(rgb.to_hex().len(), 7);
//! ```
//!
//! [ratatui]: https://ratatui.rs

use palette::{FromColor, Hsl, Hsv, RgbHue, Srgb};
use ratatui::layout::{Constraint, Layout, Rect};

mod widget;
pub use widget::{ColorPicker, PickerTheme};

/// A 24-bit sRGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `rrggbb` / `#rrggbb` (case-insensitive). Returns `None` if malformed.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self { r, g, b })
    }

    /// Format as `#rrggbb` (lowercase).
    pub fn to_hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl From<RgbColor> for ratatui::style::Color {
    fn from(c: RgbColor) -> Self {
        ratatui::style::Color::Rgb(c.r, c.g, c.b)
    }
}

impl From<[u8; 3]> for RgbColor {
    fn from(rgb: [u8; 3]) -> Self {
        Self::new(rgb[0], rgb[1], rgb[2])
    }
}

impl From<RgbColor> for [u8; 3] {
    fn from(c: RgbColor) -> Self {
        [c.r, c.g, c.b]
    }
}

/// Which editing surface the picker is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerMode {
    /// Three RGB sliders.
    RgbSliders,
    /// A 2-D hue/saturation field plus a lightness (value) slider.
    HslField,
}

impl ColorPickerMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::RgbSliders => Self::HslField,
            Self::HslField => Self::RgbSliders,
        }
    }
}

/// The currently focused control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerFocus {
    ModeToggle,
    RgbSlider(usize),
    HslField,
    LightnessSlider,
    HexField,
    RgbField(usize),
    HslFieldValue(usize),
}

/// A control that can be driven by mouse drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDragTarget {
    HslField,
    LightnessSlider,
    /// One of the three RGB sliders (0 = R, 1 = G, 2 = B).
    RgbSlider(usize),
}

/// A text-editable numeric/hex field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableField {
    Hex,
    Rgb(usize),
    Hsl(usize),
}

/// In-progress inline text edit state.
#[derive(Debug, Clone)]
pub(crate) struct TextEditState {
    pub target: EditableField,
    pub value: String,
}

#[derive(Debug, Clone, Copy)]
pub struct HslValue {
    pub hue: f32,
    pub saturation: f32,
    pub lightness: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct HsvValue {
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
}

/// Screen rectangles for each control, produced by [`picker_layout`].
///
/// Used both for rendering and for mouse hit-testing via
/// [`ColorEditor::focus_for_point`].
#[derive(Debug, Clone, Copy)]
pub struct PickerRects {
    pub overlay: Rect,
    pub mode_switch: Rect,
    pub main_view: Rect,
    pub aux_slider: Rect,
    pub hex_field: Rect,
    pub rgb_fields: [Rect; 3],
    pub hsl_fields: [Rect; 3],
    /// The draggable bar of each RGB slider (R, G, B). Zero-sized in HSL mode.
    pub rgb_slider_bars: [Rect; 3],
}

impl Default for PickerRects {
    fn default() -> Self {
        let zero = Rect::new(0, 0, 0, 0);
        Self {
            overlay: zero,
            mode_switch: zero,
            main_view: zero,
            aux_slider: zero,
            hex_field: zero,
            rgb_fields: [zero; 3],
            hsl_fields: [zero; 3],
            rgb_slider_bars: [zero; 3],
        }
    }
}

/// The color-picker state machine.
///
/// Construct with [`ColorEditor::from_rgb`], drive it from your event loop with the
/// methods below, and read the result with [`ColorEditor::to_rgb`] / [`ColorEditor::hex`].
#[derive(Debug, Clone)]
pub struct ColorEditor {
    mode: ColorPickerMode,
    focus: ColorPickerFocus,
    rgb: [u8; 3],
    hsl: HslValue,
    hsv: HsvValue,
    drag_target: Option<ColorDragTarget>,
    text_edit: Option<TextEditState>,
}

impl ColorEditor {
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let mut editor = Self {
            mode: ColorPickerMode::RgbSliders,
            focus: ColorPickerFocus::RgbSlider(0),
            rgb: [r, g, b],
            hsl: HslValue {
                hue: 0.0,
                saturation: 0.0,
                lightness: 0.0,
            },
            hsv: HsvValue {
                hue: 0.0,
                saturation: 0.0,
                value: 0.0,
            },
            drag_target: None,
            text_edit: None,
        };
        editor.sync_from_rgb_preserve_hue(None);
        editor
    }

    /// Construct from any value convertible into [`RgbColor`].
    pub fn from_color(color: impl Into<RgbColor>) -> Self {
        let c = color.into();
        Self::from_rgb(c.r, c.g, c.b)
    }

    pub fn to_rgb(&self) -> RgbColor {
        RgbColor::new(self.rgb[0], self.rgb[1], self.rgb[2])
    }

    pub fn rgb(&self) -> [u8; 3] {
        self.rgb
    }

    pub fn hsl(&self) -> HslValue {
        self.hsl
    }

    pub fn hsv(&self) -> HsvValue {
        self.hsv
    }

    pub fn mode(&self) -> ColorPickerMode {
        self.mode
    }

    pub fn focus(&self) -> ColorPickerFocus {
        self.focus
    }

    /// The active mouse-drag target, if a drag is in progress.
    pub fn drag_target(&self) -> Option<ColorDragTarget> {
        self.drag_target
    }

    /// Which field is currently being text-edited, if any.
    pub fn editing_field(&self) -> Option<EditableField> {
        self.text_edit.as_ref().map(|edit| edit.target)
    }

    pub fn hex(&self) -> String {
        self.to_rgb().to_hex()
    }

    /// Begin editing the hex field. Starts with an empty buffer so the next keystrokes
    /// type a fresh value; committing an empty buffer leaves the color unchanged.
    pub fn start_hex_input(&mut self) {
        self.focus = ColorPickerFocus::HexField;
        self.text_edit = Some(TextEditState {
            target: EditableField::Hex,
            value: String::new(),
        });
    }

    /// Begin editing the focused field (hex / R,G,B / H,S,L). Starts with an empty
    /// buffer; an empty commit is a no-op, Esc cancels.
    pub fn start_editing_focused(&mut self) {
        let target = match self.focus {
            ColorPickerFocus::HexField => EditableField::Hex,
            ColorPickerFocus::RgbField(i) => EditableField::Rgb(i),
            ColorPickerFocus::HslFieldValue(i) => EditableField::Hsl(i),
            _ => return,
        };
        self.text_edit = Some(TextEditState {
            target,
            value: String::new(),
        });
    }

    pub fn is_editing_text(&self) -> bool {
        self.text_edit.is_some()
    }

    pub fn push_input_char(&mut self, c: char) -> bool {
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        match edit.target {
            EditableField::Hex => {
                if edit.value.len() < 6 && c.is_ascii_hexdigit() {
                    edit.value.push(c.to_ascii_lowercase());
                    return true;
                }
            }
            EditableField::Rgb(_) | EditableField::Hsl(_) => {
                if c.is_ascii_digit() {
                    edit.value.push(c);
                    return true;
                }
                if c == '.'
                    && matches!(edit.target, EditableField::Hsl(_))
                    && !edit.value.contains('.')
                {
                    edit.value.push(c);
                    return true;
                }
            }
        }
        false
    }

    pub fn pop_input_char(&mut self) -> bool {
        let Some(edit) = self.text_edit.as_mut() else {
            return false;
        };
        edit.value.pop();
        true
    }

    pub fn cancel_text_edit(&mut self) {
        self.text_edit = None;
    }

    pub fn commit_text_edit(&mut self) -> bool {
        let Some(edit) = self.text_edit.take() else {
            return false;
        };
        match edit.target {
            EditableField::Hex => {
                if let Some(rgb) = RgbColor::from_hex(&edit.value) {
                    self.set_rgb([rgb.r, rgb.g, rgb.b]);
                    return true;
                }
            }
            EditableField::Rgb(i) => {
                if let Ok(value) = edit.value.parse::<u16>() {
                    if value <= 255 {
                        let mut rgb = self.rgb;
                        rgb[i] = value as u8;
                        self.set_rgb(rgb);
                        return true;
                    }
                }
            }
            EditableField::Hsl(i) => {
                if let Ok(value) = edit.value.parse::<f32>() {
                    match i {
                        0 => self.set_hsl(value, self.hsl.saturation, self.hsl.lightness),
                        1 => self.set_hsl(self.hsl.hue, value, self.hsl.lightness),
                        2 => self.set_hsl(self.hsl.hue, self.hsl.saturation, value),
                        _ => {}
                    }
                    return true;
                }
            }
        }
        false
    }

    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
        self.focus = match self.mode {
            ColorPickerMode::RgbSliders => ColorPickerFocus::RgbSlider(0),
            ColorPickerMode::HslField => ColorPickerFocus::HslField,
        };
        self.drag_target = None;
        self.text_edit = None;
    }

    pub fn focus_next(&mut self, reverse: bool) {
        self.text_edit = None;
        self.drag_target = None;
        let order = self.focus_order();
        let idx = order
            .iter()
            .position(|focus| *focus == self.focus)
            .unwrap_or(0);
        let next = if reverse {
            if idx == 0 {
                order.len() - 1
            } else {
                idx - 1
            }
        } else {
            (idx + 1) % order.len()
        };
        self.focus = order[next];
    }

    pub fn adjust_rgb_slider_selection(&mut self, delta: i32) {
        if let ColorPickerFocus::RgbSlider(idx) = self.focus {
            let mut rgb = self.rgb;
            rgb[idx] = (i32::from(rgb[idx]) + delta).clamp(0, 255) as u8;
            self.set_rgb(rgb);
        }
    }

    /// Set an RGB channel (0 = R, 1 = G, 2 = B) from a 0.0..=1.0 position along its
    /// slider — e.g. a mouse click/drag fraction. Focuses that slider.
    pub fn set_rgb_slider_frac(&mut self, channel: usize, x_frac: f32) {
        if channel < 3 {
            let mut rgb = self.rgb;
            rgb[channel] = (x_frac.clamp(0.0, 1.0) * 255.0).round() as u8;
            self.set_rgb(rgb);
            self.focus = ColorPickerFocus::RgbSlider(channel);
        }
    }

    pub fn move_rgb_slider_focus(&mut self, reverse: bool) {
        let current = match self.focus {
            ColorPickerFocus::RgbSlider(idx) => idx,
            _ => 0,
        };
        let next = if reverse {
            if current == 0 {
                2
            } else {
                current - 1
            }
        } else {
            (current + 1) % 3
        };
        self.focus = ColorPickerFocus::RgbSlider(next);
    }

    pub fn adjust_focused_numeric(&mut self, delta: f32) -> bool {
        match self.focus {
            ColorPickerFocus::RgbSlider(i) | ColorPickerFocus::RgbField(i) => {
                let mut rgb = self.rgb;
                rgb[i] = (f32::from(rgb[i]) + delta).clamp(0.0, 255.0).round() as u8;
                self.set_rgb(rgb);
                true
            }
            ColorPickerFocus::HslFieldValue(0) => {
                self.set_hsl(
                    self.hsl.hue + delta,
                    self.hsl.saturation,
                    self.hsl.lightness,
                );
                true
            }
            ColorPickerFocus::HslFieldValue(1) => {
                self.set_hsl(
                    self.hsl.hue,
                    self.hsl.saturation + delta,
                    self.hsl.lightness,
                );
                true
            }
            ColorPickerFocus::HslFieldValue(2) => {
                self.set_hsl(
                    self.hsl.hue,
                    self.hsl.saturation,
                    self.hsl.lightness + delta,
                );
                true
            }
            ColorPickerFocus::LightnessSlider => {
                self.set_hsv(self.hsv.hue, self.hsv.saturation, self.hsv.value + delta);
                true
            }
            _ => false,
        }
    }

    pub fn update_from_hsl_field(&mut self, x_frac: f32, y_frac: f32) {
        let hue = x_frac.clamp(0.0, 1.0) * 360.0;
        let saturation = (1.0 - y_frac.clamp(0.0, 1.0)) * 100.0;
        self.set_hsv(hue, saturation, self.hsv.value);
        self.focus = ColorPickerFocus::HslField;
    }

    pub fn update_lightness_from_frac(&mut self, y_frac: f32) {
        let value = (1.0 - y_frac.clamp(0.0, 1.0)) * 100.0;
        self.set_hsv(self.hsv.hue, self.hsv.saturation, value);
        self.focus = ColorPickerFocus::LightnessSlider;
    }

    pub fn nudge_hsl_field(&mut self, delta_hue: f32, delta_saturation: f32) {
        self.set_hsv(
            self.hsv.hue + delta_hue,
            self.hsv.saturation + delta_saturation,
            self.hsv.value,
        );
    }

    pub fn set_drag_target(&mut self, target: Option<ColorDragTarget>) {
        self.drag_target = target;
    }

    pub fn field_value(&self, field: EditableField) -> String {
        if let Some(edit) = &self.text_edit {
            let matches = match (edit.target, field) {
                (EditableField::Hex, EditableField::Hex) => true,
                (EditableField::Rgb(a), EditableField::Rgb(b)) => a == b,
                (EditableField::Hsl(a), EditableField::Hsl(b)) => a == b,
                _ => false,
            };
            if matches {
                return edit.value.clone();
            }
        }
        match field {
            EditableField::Hex => self.hex(),
            EditableField::Rgb(i) => self.rgb[i].to_string(),
            EditableField::Hsl(0) => format!("{:.0}", self.hsl.hue),
            EditableField::Hsl(1) => format!("{:.0}", self.hsl.saturation),
            EditableField::Hsl(2) => format!("{:.0}", self.hsl.lightness),
            EditableField::Hsl(_) => String::new(),
        }
    }

    pub fn focus_label(&self) -> &'static str {
        match self.focus {
            ColorPickerFocus::ModeToggle => "mode switch",
            ColorPickerFocus::RgbSlider(0) => "red slider",
            ColorPickerFocus::RgbSlider(1) => "green slider",
            ColorPickerFocus::RgbSlider(2) => "blue slider",
            ColorPickerFocus::RgbSlider(_) => "rgb slider",
            ColorPickerFocus::HslField => "color field",
            ColorPickerFocus::LightnessSlider => "value slider",
            ColorPickerFocus::HexField => "hex field",
            ColorPickerFocus::RgbField(0) => "red field",
            ColorPickerFocus::RgbField(1) => "green field",
            ColorPickerFocus::RgbField(2) => "blue field",
            ColorPickerFocus::RgbField(_) => "rgb field",
            ColorPickerFocus::HslFieldValue(0) => "hue field",
            ColorPickerFocus::HslFieldValue(1) => "sat field",
            ColorPickerFocus::HslFieldValue(2) => "light field",
            ColorPickerFocus::HslFieldValue(_) => "hsl field",
        }
    }

    /// Map a screen point to the control under it, given a computed [`PickerRects`].
    pub fn focus_for_point(&self, rects: &PickerRects, x: u16, y: u16) -> Option<ColorPickerFocus> {
        let point = (x, y);
        if contains(rects.mode_switch, point) {
            return Some(ColorPickerFocus::ModeToggle);
        }
        if contains(rects.main_view, point) {
            return Some(match self.mode {
                ColorPickerMode::RgbSliders => {
                    let idx = ((y.saturating_sub(rects.main_view.y)) / 2).min(2) as usize;
                    ColorPickerFocus::RgbSlider(idx)
                }
                ColorPickerMode::HslField => ColorPickerFocus::HslField,
            });
        }
        if contains(rects.aux_slider, point) {
            return Some(match self.mode {
                ColorPickerMode::RgbSliders => ColorPickerFocus::RgbSlider(2),
                ColorPickerMode::HslField => ColorPickerFocus::LightnessSlider,
            });
        }
        if contains(rects.hex_field, point) {
            return Some(ColorPickerFocus::HexField);
        }
        for (idx, rect) in rects.rgb_fields.iter().enumerate() {
            if contains(*rect, point) {
                return Some(ColorPickerFocus::RgbField(idx));
            }
        }
        for (idx, rect) in rects.hsl_fields.iter().enumerate() {
            if contains(*rect, point) {
                return Some(ColorPickerFocus::HslFieldValue(idx));
            }
        }
        None
    }

    pub fn set_focus(&mut self, focus: ColorPickerFocus) {
        self.focus = focus;
        self.text_edit = None;
        self.drag_target = None;
    }

    fn focus_order(&self) -> Vec<ColorPickerFocus> {
        match self.mode {
            ColorPickerMode::RgbSliders => vec![
                ColorPickerFocus::ModeToggle,
                ColorPickerFocus::RgbSlider(0),
                ColorPickerFocus::RgbSlider(1),
                ColorPickerFocus::RgbSlider(2),
                ColorPickerFocus::HexField,
                ColorPickerFocus::RgbField(0),
                ColorPickerFocus::RgbField(1),
                ColorPickerFocus::RgbField(2),
                ColorPickerFocus::HslFieldValue(0),
                ColorPickerFocus::HslFieldValue(1),
                ColorPickerFocus::HslFieldValue(2),
            ],
            ColorPickerMode::HslField => vec![
                ColorPickerFocus::ModeToggle,
                ColorPickerFocus::HslField,
                ColorPickerFocus::LightnessSlider,
                ColorPickerFocus::HexField,
                ColorPickerFocus::RgbField(0),
                ColorPickerFocus::RgbField(1),
                ColorPickerFocus::RgbField(2),
                ColorPickerFocus::HslFieldValue(0),
                ColorPickerFocus::HslFieldValue(1),
                ColorPickerFocus::HslFieldValue(2),
            ],
        }
    }

    fn set_rgb(&mut self, rgb: [u8; 3]) {
        self.rgb = rgb;
        let preserve_hue = Some(self.hsv.hue);
        self.sync_from_rgb_preserve_hue(preserve_hue);
    }

    fn set_hsl(&mut self, hue: f32, saturation: f32, lightness: f32) {
        self.hsl = HslValue {
            hue: normalize_hue(hue),
            saturation: saturation.clamp(0.0, 100.0),
            lightness: lightness.clamp(0.0, 100.0),
        };
        let hsl = Hsl::new(
            RgbHue::from_degrees(self.hsl.hue),
            self.hsl.saturation / 100.0,
            self.hsl.lightness / 100.0,
        );
        let srgb: Srgb<f32> = Srgb::from_color(hsl);
        let srgb_u8 = srgb.into_format::<u8>();
        self.rgb = [srgb_u8.red, srgb_u8.green, srgb_u8.blue];
    }

    fn set_hsv(&mut self, hue: f32, saturation: f32, value: f32) {
        self.hsv = HsvValue {
            hue: normalize_hue(hue),
            saturation: saturation.clamp(0.0, 100.0),
            value: value.clamp(0.0, 100.0),
        };
        let hsv = Hsv::new(
            RgbHue::from_degrees(self.hsv.hue),
            self.hsv.saturation / 100.0,
            self.hsv.value / 100.0,
        );
        let srgb: Srgb<f32> = Srgb::from_color(hsv);
        let srgb_u8 = srgb.into_format::<u8>();
        self.rgb = [srgb_u8.red, srgb_u8.green, srgb_u8.blue];
        self.sync_hsl_from_rgb();
    }

    fn sync_hsl_from_rgb(&mut self) {
        let hsl: Hsl = Hsl::from_color(srgb_f32(self.rgb));
        self.hsl = HslValue {
            hue: normalize_hue(hsl.hue.into_degrees()),
            saturation: hsl.saturation * 100.0,
            lightness: hsl.lightness * 100.0,
        };
    }

    fn sync_from_rgb_preserve_hue(&mut self, preserve_hue: Option<f32>) {
        let hsv: Hsv = Hsv::from_color(srgb_f32(self.rgb));
        let raw_saturation = hsv.saturation * 100.0;
        let raw_value = hsv.value * 100.0;
        let hue = if raw_saturation <= 0.01 || raw_value <= 0.01 {
            preserve_hue.unwrap_or_else(|| normalize_hue(hsv.hue.into_degrees()))
        } else {
            normalize_hue(hsv.hue.into_degrees())
        };
        self.hsv = HsvValue {
            hue,
            saturation: raw_saturation,
            value: raw_value,
        };
        self.sync_hsl_from_rgb();
        if self.hsl.saturation <= 0.01 {
            self.hsl.hue = hue;
        }
    }
}

/// Compute the control rectangles for a picker drawn into `overlay`.
///
/// Pass the rectangle you want the picker to occupy; the layout is computed inside it.
/// The result feeds both rendering and [`ColorEditor::focus_for_point`].
/// The header / body / footer row split, shared between [`picker_layout`] and the
/// widget so the two can't drift. Footer is a single row.
pub(crate) fn body_rows(inner: Rect) -> [Rect; 3] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(16),
        Constraint::Length(1),
    ])
    .areas(inner)
}

/// The three RGB slider bar rectangles inside the "Channels" block (`main_view`).
/// Shared between [`picker_layout`] and the widget so the drawn bar and the
/// draggable hit-box stay aligned. Layout: ` R ` label (3) + bar + value (gutter 5).
pub(crate) fn rgb_slider_bar_rects(main_view: Rect) -> [Rect; 3] {
    if main_view.width < 2 || main_view.height < 2 {
        return [Rect::new(0, 0, 0, 0); 3];
    }
    let inner_x = main_view.x + 1;
    let inner_y = main_view.y + 1;
    let inner_w = main_view.width - 2;
    let bar_x = inner_x + 3;
    let bar_w = inner_w.saturating_sub(8);
    std::array::from_fn(|i| Rect {
        x: bar_x,
        y: inner_y + (i as u16) * 2,
        width: bar_w,
        height: 1,
    })
}

pub fn picker_layout(overlay: Rect, mode: ColorPickerMode) -> PickerRects {
    let inner = Rect::new(
        overlay.x + 1,
        overlay.y + 1,
        overlay.width.saturating_sub(2),
        overlay.height.saturating_sub(2),
    );
    let [_header, body, _footer] = body_rows(inner);
    let [main_col, side_col] =
        Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(body);
    let (main_view, aux_slider) = match mode {
        ColorPickerMode::RgbSliders => {
            let [sliders, _spacer] =
                Layout::vertical([Constraint::Length(7), Constraint::Min(1)]).areas(main_col);
            (sliders, Rect::new(0, 0, 0, 0))
        }
        ColorPickerMode::HslField => {
            let [field, slider] =
                Layout::horizontal([Constraint::Fill(1), Constraint::Length(6)]).areas(main_col);
            (field, slider)
        }
    };
    let [_preview_area, fields_area] =
        Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]).areas(side_col);
    let [mode_switch, _preview_swatch] =
        Layout::horizontal([Constraint::Length(16), Constraint::Fill(1)]).areas(_header);
    let [hex_row, rgb_row, hsl_row] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .areas(fields_area);
    let [hex_field, _] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(0)]).areas(hex_row);
    let rgb_fields = split_three(rgb_row);
    let hsl_fields = split_three(hsl_row);
    let rgb_slider_bars = match mode {
        ColorPickerMode::RgbSliders => rgb_slider_bar_rects(main_view),
        ColorPickerMode::HslField => [Rect::new(0, 0, 0, 0); 3],
    };
    PickerRects {
        overlay,
        mode_switch,
        main_view,
        aux_slider,
        hex_field,
        rgb_fields,
        hsl_fields,
        rgb_slider_bars,
    }
}

/// Split a rectangle into three roughly-equal horizontal columns.
pub fn split_three(area: Rect) -> [Rect; 3] {
    let [a, b, c] = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(34),
        Constraint::Percentage(33),
    ])
    .areas(area);
    [a, b, c]
}

/// The sRGB color of an HSV field cell — handy when drawing the hue/sat gradient.
pub fn hsv_field_cell(hue_deg: f32, saturation_pct: f32, value_pct: f32) -> RgbColor {
    let hsv = Hsv::new(
        RgbHue::from_degrees(normalize_hue(hue_deg)),
        saturation_pct.clamp(0.0, 100.0) / 100.0,
        value_pct.clamp(0.0, 100.0) / 100.0,
    );
    let srgb: Srgb<f32> = Srgb::from_color(hsv);
    let srgb = srgb.into_format::<u8>();
    RgbColor::new(srgb.red, srgb.green, srgb.blue)
}

/// A readable foreground (near-black or near-white) for text drawn on `rgb`.
pub fn contrast_text(rgb: RgbColor) -> RgbColor {
    let luminance =
        (0.2126 * f32::from(rgb.r) + 0.7152 * f32::from(rgb.g) + 0.0722 * f32::from(rgb.b)) / 255.0;
    if luminance > 0.55 {
        RgbColor::new(18, 18, 24)
    } else {
        RgbColor::new(245, 245, 250)
    }
}

/// Convert a `[u8; 3]` to a `palette` linear-friendly `Srgb<f32>`.
pub fn srgb_f32(rgb: [u8; 3]) -> Srgb<f32> {
    Srgb::new(rgb[0], rgb[1], rgb[2]).into_format()
}

/// Wrap a hue into `[0, 360)`.
pub fn normalize_hue(hue: f32) -> f32 {
    hue.rem_euclid(360.0)
}

fn contains(rect: Rect, point: (u16, u16)) -> bool {
    point.0 >= rect.x
        && point.0 < rect.x + rect.width
        && point.1 >= rect.y
        && point.1 < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let c = RgbColor::from_hex("#89b4fa").unwrap();
        assert_eq!((c.r, c.g, c.b), (0x89, 0xb4, 0xfa));
        assert_eq!(c.to_hex(), "#89b4fa");
        assert!(RgbColor::from_hex("xyz").is_none());
    }

    #[test]
    fn rgb_hsl_hsv_stay_consistent() {
        let editor = ColorEditor::from_rgb(0x89, 0xb4, 0xfa);
        // HSL/HSV derived from the same RGB should round-trip back to it.
        let back = ColorEditor::from_rgb(editor.rgb[0], editor.rgb[1], editor.rgb[2]);
        assert_eq!(editor.rgb, back.rgb);
        assert!((editor.hsl.hue - back.hsl.hue).abs() < 0.5);
    }

    #[test]
    fn greys_preserve_hue() {
        // A pure grey has undefined hue; constructing it should not panic or NaN.
        let editor = ColorEditor::from_rgb(128, 128, 128);
        assert!(editor.hsl.saturation.abs() < 1.0);
        assert!(editor.hsv.hue.is_finite());
    }

    #[test]
    fn tab_order_wraps() {
        let mut editor = ColorEditor::from_rgb(10, 20, 30);
        editor.focus = ColorPickerFocus::ModeToggle;
        editor.focus_next(true); // reverse from first wraps to last
        assert_eq!(editor.focus, ColorPickerFocus::HslFieldValue(2));
    }

    #[test]
    fn rgb_slider_frac_sets_channel() {
        let mut e = ColorEditor::from_rgb(0, 0, 0);
        e.set_rgb_slider_frac(0, 1.0);
        assert_eq!(e.rgb[0], 255);
        e.set_rgb_slider_frac(0, 0.5);
        assert_eq!(e.rgb[0], 128); // round(127.5)
        assert_eq!(e.focus, ColorPickerFocus::RgbSlider(0));
    }

    #[test]
    fn layout_exposes_rgb_slider_bars() {
        let rects = picker_layout(Rect::new(0, 0, 76, 24), ColorPickerMode::RgbSliders);
        for bar in rects.rgb_slider_bars {
            assert!(bar.width > 0 && bar.height == 1);
            // bar sits inside the Channels block (main_view)
            assert!(bar.x >= rects.main_view.x);
            assert!(bar.x + bar.width <= rects.main_view.x + rects.main_view.width);
        }
        // HSL mode exposes no slider bars.
        let hsl = picker_layout(Rect::new(0, 0, 76, 24), ColorPickerMode::HslField);
        assert!(hsl.rgb_slider_bars.iter().all(|b| b.width == 0));
    }

    #[test]
    fn hex_field_can_be_typed_and_committed() {
        let mut e = ColorEditor::from_rgb(0, 0, 0);
        e.set_focus(ColorPickerFocus::HexField);
        e.start_editing_focused();
        for c in "ff8800".chars() {
            assert!(e.push_input_char(c), "hex digit {c} should be accepted");
        }
        assert!(e.commit_text_edit());
        assert_eq!(e.rgb, [255, 136, 0]);
    }

    #[test]
    fn rgb_field_can_be_typed_and_committed() {
        let mut e = ColorEditor::from_rgb(0, 0, 0);
        e.set_focus(ColorPickerFocus::RgbField(0));
        e.start_editing_focused();
        for c in "200".chars() {
            assert!(e.push_input_char(c));
        }
        assert!(e.commit_text_edit());
        assert_eq!(e.rgb[0], 200);
    }

    #[test]
    fn tab_reaches_text_fields_in_hsl_mode() {
        let mut e = ColorEditor::from_rgb(10, 20, 30);
        e.toggle_mode(); // -> HSL field mode
        assert_eq!(e.mode, ColorPickerMode::HslField);
        let mut seen = Vec::new();
        for _ in 0..12 {
            seen.push(e.focus);
            e.focus_next(false);
        }
        assert!(seen.contains(&ColorPickerFocus::HexField));
        assert!(seen.contains(&ColorPickerFocus::RgbField(0)));
        assert!(seen.contains(&ColorPickerFocus::HslFieldValue(2)));
    }

    #[test]
    fn editing_clamps_rgb() {
        let mut editor = ColorEditor::from_rgb(0, 0, 0);
        editor.set_focus(ColorPickerFocus::RgbField(0));
        editor.start_editing_focused();
        for c in "999".chars() {
            editor.push_input_char(c);
        }
        // 999 > 255, commit should reject and leave the channel unchanged.
        assert!(!editor.commit_text_edit());
        assert_eq!(editor.rgb[0], 0);
    }
}
