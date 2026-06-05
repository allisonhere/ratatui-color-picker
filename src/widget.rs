//! A batteries-included [`StatefulWidget`] that draws the whole picker.
//!
//! ```no_run
//! use ratatui::widgets::StatefulWidget;
//! use ratatui_color_picker::{ColorEditor, ColorPicker};
//! # fn draw(frame: &mut ratatui::Frame, editor: &mut ColorEditor) {
//! frame.render_stateful_widget(ColorPicker::new(), frame.area(), editor);
//! # }
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, StatefulWidget, Widget};

use crate::{
    contrast_text, hsv_field_cell, picker_layout, ColorEditor, ColorPickerFocus, ColorPickerMode,
    EditableField, RgbColor,
};

/// The color palette the [`ColorPicker`] widget draws with. [`Default`] matches the
/// look the picker shipped with; override any field to retheme it.
#[derive(Debug, Clone, Copy)]
pub struct PickerTheme {
    pub bg: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub dim: Color,
    pub accent_bg: Color,
    pub accent_fg: Color,
    pub subtle_bg: Color,
    pub subtle_fg: Color,
    pub surface_bg: Color,
    pub surface_focus_bg: Color,
}

impl Default for PickerTheme {
    fn default() -> Self {
        Self {
            bg: Color::Rgb(22, 22, 26),
            border: Color::Rgb(90, 85, 115),
            text: Color::Rgb(212, 212, 230),
            muted: Color::Rgb(120, 120, 145),
            dim: Color::Rgb(84, 84, 104),
            accent_bg: Color::Rgb(97, 88, 150),
            accent_fg: Color::Rgb(242, 240, 255),
            subtle_bg: Color::Rgb(54, 50, 74),
            subtle_fg: Color::Rgb(214, 210, 235),
            surface_bg: Color::Rgb(26, 26, 32),
            surface_focus_bg: Color::Rgb(34, 31, 46),
        }
    }
}

/// A ready-to-render color picker widget. State is a [`ColorEditor`].
#[derive(Debug, Clone, Default)]
pub struct ColorPicker {
    theme: PickerTheme,
    original: Option<RgbColor>,
}

impl ColorPicker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the color palette.
    pub fn theme(mut self, theme: PickerTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Show a "before → after" swatch comparing against the original color.
    pub fn original(mut self, original: Option<RgbColor>) -> Self {
        self.original = original;
        self
    }
}

fn tui(c: RgbColor) -> Color {
    c.into()
}

impl StatefulWidget for ColorPicker {
    type State = ColorEditor;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let t = self.theme;
        let rects = picker_layout(area, state.mode);
        Clear.render(rects.overlay, buf);

        let outer = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(t.border))
            .style(Style::new().bg(t.bg));
        let inner = outer.inner(rects.overlay);
        outer.render(rects.overlay, buf);

        let [header, body, footer] = crate::body_rows(inner);
        let [main_col, side_col] =
            Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)]).areas(body);
        let [preview_area, _fields_area] =
            Layout::vertical([Constraint::Length(5), Constraint::Fill(1)]).areas(side_col);

        let current_rgb = state.to_rgb();
        let current_hex = state.hex();
        let hsl = state.hsl;
        let hsv = state.hsv();

        // ── Header: title + RGB/HSL mode pills ──────────────────────────────
        let mk_pill = |key: &str, label: &str, active: bool| -> Vec<Span<'static>> {
            let (key_bg, key_fg, lbl_bg, lbl_fg) = if active {
                (t.accent_bg, t.accent_fg, t.subtle_bg, t.subtle_fg)
            } else {
                (t.subtle_bg, t.subtle_fg, t.bg, t.muted)
            };
            vec![
                Span::styled("\u{e0b6}", Style::new().fg(key_bg).bg(t.bg)),
                Span::styled(
                    format!(" {} ", key),
                    Style::new().fg(key_fg).bg(key_bg).add_modifier(Modifier::BOLD),
                ),
                Span::styled("\u{e0b0}", Style::new().fg(lbl_bg).bg(key_bg)),
                Span::styled(format!(" {} ", label), Style::new().fg(lbl_fg).bg(lbl_bg)),
                Span::styled("\u{e0b4}", Style::new().fg(lbl_bg).bg(t.bg)),
            ]
        };
        let mode_focused = state.focus == ColorPickerFocus::ModeToggle;
        let mut header_spans = vec![Span::styled(
            " Color Picker ",
            Style::new().fg(t.text).add_modifier(Modifier::BOLD),
        )];
        if mode_focused {
            header_spans.push(Span::styled(
                "\u{203a} ",
                Style::new().fg(t.accent_fg).add_modifier(Modifier::BOLD),
            ));
        }
        header_spans.extend(mk_pill("M", "rgb", state.mode == ColorPickerMode::RgbSliders));
        header_spans.push(Span::raw(" "));
        header_spans.extend(mk_pill("M", "hsl", state.mode == ColorPickerMode::HslField));
        Paragraph::new(Line::from(header_spans))
            .style(Style::new().bg(t.bg))
            .render(header, buf);

        Block::default()
            .style(Style::new().bg(t.surface_bg))
            .render(main_col, buf);
        Block::default().style(Style::new().bg(t.bg)).render(side_col, buf);

        // ── Main view ───────────────────────────────────────────────────────
        match state.mode {
            ColorPickerMode::RgbSliders => {
                let channels_focus = matches!(state.focus, ColorPickerFocus::RgbSlider(_));
                let channels_block = Block::bordered()
                    .title(" Channels ")
                    .title_style(Style::new().fg(t.text).add_modifier(Modifier::BOLD))
                    .border_type(BorderType::Rounded)
                    .border_style(Style::new().fg(if channels_focus { t.accent_bg } else { t.border }))
                    .style(Style::new().bg(t.surface_bg));
                let channels_inner = channels_block.inner(rects.main_view);
                channels_block.render(rects.main_view, buf);
                let slider_width = channels_inner.width.saturating_sub(8) as usize;
                for (idx, label) in ["R", "G", "B"].into_iter().enumerate() {
                    let row_rect = Rect {
                        x: channels_inner.x,
                        y: channels_inner.y + (idx as u16 * 2),
                        width: channels_inner.width,
                        height: 1,
                    };
                    let value = state.rgb[idx];
                    let filled = ((value as f32 / 255.0) * slider_width as f32).round() as usize;
                    let bar: String = (0..slider_width)
                        .map(|i| if i < filled { '\u{2588}' } else { '\u{2591}' })
                        .collect();
                    let is_focus = state.focus == ColorPickerFocus::RgbSlider(idx);
                    let color = match idx {
                        0 => Color::Rgb(255, 96, 96),
                        1 => Color::Rgb(106, 220, 124),
                        _ => Color::Rgb(102, 186, 255),
                    };
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            format!(" {} ", label),
                            Style::new()
                                .fg(if is_focus { t.accent_fg } else { t.text })
                                .bg(if is_focus { t.accent_bg } else { t.surface_bg })
                                .add_modifier(if is_focus { Modifier::BOLD } else { Modifier::empty() }),
                        ),
                        Span::styled(bar, Style::new().fg(color).bg(t.surface_bg)),
                        Span::styled(
                            format!(" {:>3}", value),
                            Style::new().fg(if is_focus { t.text } else { t.muted }),
                        ),
                    ]))
                    .style(Style::new().bg(t.surface_bg))
                    .render(row_rect, buf);
                }
                Paragraph::new(vec![
                    Line::from(""),
                    Line::from(Span::styled(
                        " RGB sliders for exact channel edits",
                        Style::new().fg(t.text).add_modifier(Modifier::BOLD),
                    )),
                    Line::from(Span::styled(
                        " Press M to switch to the HSL field picker.",
                        Style::new().fg(t.muted),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!(" Current  {}", current_hex),
                        Style::new().fg(t.text),
                    )),
                    Line::from(Span::styled(
                        format!(" HSV {:.0} / {:.0}% / {:.0}%", hsv.hue, hsv.saturation, hsv.value),
                        Style::new().fg(t.muted),
                    )),
                ])
                .style(Style::new().bg(t.surface_bg))
                .render(
                    Rect {
                        x: channels_inner.x,
                        y: channels_inner.y + 6,
                        width: channels_inner.width,
                        height: channels_inner.height.saturating_sub(6),
                    },
                    buf,
                );
            }
            ColorPickerMode::HslField => {
                let field_focus = state.focus == ColorPickerFocus::HslField;
                let field_block = Block::bordered()
                    .title(if field_focus { " Color Field \u{25cf} " } else { " Color Field " })
                    .title_style(
                        Style::new()
                            .fg(t.text)
                            .add_modifier(if field_focus { Modifier::BOLD } else { Modifier::empty() }),
                    )
                    .border_type(if field_focus { BorderType::Double } else { BorderType::Rounded })
                    .border_style(Style::new().fg(if field_focus { t.accent_bg } else { t.border }))
                    .style(Style::new().bg(if field_focus { t.surface_focus_bg } else { t.surface_bg }));
                let field_area = field_block.inner(rects.main_view);
                field_block.render(rects.main_view, buf);
                for row in 0..field_area.height {
                    let mut spans = Vec::with_capacity(field_area.width as usize);
                    for col in 0..field_area.width {
                        let x_frac = col as f32 / field_area.width.saturating_sub(1).max(1) as f32;
                        let top_frac =
                            (row as f32 * 2.0) / (field_area.height.max(1) as f32 * 2.0 - 1.0);
                        let bottom_frac =
                            ((row as f32 * 2.0) + 1.0) / (field_area.height.max(1) as f32 * 2.0 - 1.0);
                        let top = hsv_field_cell(x_frac * 360.0, (1.0 - top_frac) * 100.0, hsv.value);
                        let bottom =
                            hsv_field_cell(x_frac * 360.0, (1.0 - bottom_frac) * 100.0, hsv.value);
                        let selected_col = ((hsv.hue / 360.0)
                            * field_area.width.saturating_sub(1).max(1) as f32)
                            .round() as u16;
                        let selected_row = (((100.0 - hsv.saturation) / 100.0)
                            * field_area.height.saturating_sub(1).max(1) as f32)
                            .round() as u16;
                        if col == selected_col && row == selected_row {
                            let marker = contrast_text(current_rgb);
                            spans.push(Span::styled(
                                "\u{25c9}",
                                Style::new()
                                    .fg(tui(marker))
                                    .bg(tui(current_rgb))
                                    .add_modifier(Modifier::BOLD),
                            ));
                        } else {
                            spans.push(Span::styled(
                                "\u{2580}",
                                Style::new().fg(tui(top)).bg(tui(bottom)),
                            ));
                        }
                    }
                    Paragraph::new(Line::from(spans))
                        .style(Style::new().bg(if field_focus { t.surface_focus_bg } else { t.surface_bg }))
                        .render(
                            Rect {
                                x: field_area.x,
                                y: field_area.y + row,
                                width: field_area.width,
                                height: 1,
                            },
                            buf,
                        );
                }
                let value_focus = state.focus == ColorPickerFocus::LightnessSlider;
                let value_block = Block::bordered()
                    .title(if value_focus { " V \u{25cf} " } else { " V " })
                    .title_style(
                        Style::new()
                            .fg(t.text)
                            .add_modifier(if value_focus { Modifier::BOLD } else { Modifier::empty() }),
                    )
                    .border_type(if value_focus { BorderType::Double } else { BorderType::Rounded })
                    .border_style(Style::new().fg(if value_focus { t.accent_bg } else { t.border }))
                    .style(Style::new().bg(if value_focus { t.surface_focus_bg } else { t.surface_bg }));
                let value_area = value_block.inner(rects.aux_slider);
                value_block.render(rects.aux_slider, buf);
                let selected_row = (((100.0 - hsv.value) / 100.0)
                    * value_area.height.saturating_sub(1).max(1) as f32)
                    .round() as u16;
                for row in 0..value_area.height {
                    let top_frac = (row as f32 * 2.0) / (value_area.height.max(1) as f32 * 2.0 - 1.0);
                    let bottom_frac =
                        ((row as f32 * 2.0) + 1.0) / (value_area.height.max(1) as f32 * 2.0 - 1.0);
                    let top_value = (1.0 - top_frac.clamp(0.0, 1.0)) * 100.0;
                    let bottom_value = (1.0 - bottom_frac.clamp(0.0, 1.0)) * 100.0;
                    let top_color = hsv_field_cell(hsv.hue, hsv.saturation, top_value);
                    let bottom_color = hsv_field_cell(hsv.hue, hsv.saturation, bottom_value);
                    let selected = row == selected_row;
                    let content = if selected {
                        "\u{2588}".repeat(value_area.width as usize)
                    } else {
                        "\u{2580}".repeat(value_area.width as usize)
                    };
                    let style = if selected {
                        Style::new()
                            .fg(tui(contrast_text(bottom_color)))
                            .bg(tui(bottom_color))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::new().fg(tui(top_color)).bg(tui(bottom_color))
                    };
                    Paragraph::new(Line::from(vec![Span::styled(content, style)])).render(
                        Rect {
                            x: value_area.x,
                            y: value_area.y + row,
                            width: value_area.width,
                            height: 1,
                        },
                        buf,
                    );
                }
            }
        }

        // ── Side column: preview swatch + readouts ──────────────────────────
        let current_fg = tui(contrast_text(current_rgb));
        let before_line = if let Some(orig) = self.original {
            Line::from(vec![
                Span::styled("      ", Style::new().bg(tui(orig))),
                Span::styled("  \u{2192}  ", Style::new().fg(t.dim).bg(t.bg)),
                Span::styled("      ", Style::new().bg(tui(current_rgb))),
            ])
        } else {
            Line::from(vec![Span::styled("      ", Style::new().bg(tui(current_rgb)))])
        };
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!(" {}", current_hex),
                Style::new().fg(t.text).add_modifier(Modifier::BOLD),
            )),
            before_line,
            Line::from(Span::styled(
                format!(" rgb {} {} {}", current_rgb.r, current_rgb.g, current_rgb.b),
                Style::new().fg(t.muted),
            )),
            Line::from(Span::styled(
                format!(" hsl {:.0} {:.0}% {:.0}%", hsl.hue, hsl.saturation, hsl.lightness),
                Style::new().fg(t.muted),
            )),
            Line::from(Span::styled(
                format!(" hsv {:.0} {:.0}% {:.0}%", hsv.hue, hsv.saturation, hsv.value),
                Style::new().fg(current_fg),
            )),
            Line::from(Span::styled(
                format!(" focus {}", state.focus_label()),
                Style::new().fg(t.accent_fg),
            )),
        ])
        .style(Style::new().bg(t.bg))
        .render(preview_area, buf);

        // ── Field boxes (hex / rgb / hsl) ───────────────────────────────────
        let field_box = |buf: &mut Buffer, rect: Rect, title: &str, focused: bool| {
            Block::bordered()
                .title(format!(" {} ", title))
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(if focused { t.accent_bg } else { t.border }))
                .style(Style::new().bg(t.bg))
                .render(rect, buf);
        };
        field_box(buf, rects.hex_field, "HEX", state.focus == ColorPickerFocus::HexField);
        for (idx, rect) in rects.rgb_fields.iter().enumerate() {
            field_box(buf, *rect, ["R", "G", "B"][idx], state.focus == ColorPickerFocus::RgbField(idx));
        }
        for (idx, rect) in rects.hsl_fields.iter().enumerate() {
            field_box(buf, *rect, ["H", "S", "L"][idx], state.focus == ColorPickerFocus::HslFieldValue(idx));
        }

        let field_value = |buf: &mut Buffer, rect: Rect, value: String, suffix: &str, editing: bool| {
            let inner = Rect {
                x: rect.x + 1,
                y: rect.y + 1,
                width: rect.width.saturating_sub(2),
                height: rect.height.saturating_sub(2),
            };
            Paragraph::new(Line::from(vec![
                Span::styled(
                    value,
                    Style::new()
                        .fg(if editing { t.accent_fg } else { t.text })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(suffix.to_string(), Style::new().fg(t.muted)),
            ]))
            .style(Style::new().bg(t.bg))
            .render(inner, buf);
        };
        let editing_target = state.text_edit.as_ref().map(|edit| edit.target);
        field_value(
            buf,
            rects.hex_field,
            state.field_value(EditableField::Hex),
            "",
            matches!(editing_target, Some(EditableField::Hex)),
        );
        for idx in 0..3 {
            field_value(
                buf,
                rects.rgb_fields[idx],
                state.field_value(EditableField::Rgb(idx)),
                "",
                matches!(editing_target, Some(EditableField::Rgb(i)) if i == idx),
            );
        }
        for idx in 0..3 {
            field_value(
                buf,
                rects.hsl_fields[idx],
                state.field_value(EditableField::Hsl(idx)),
                if idx == 0 { "\u{b0}" } else { "%" },
                matches!(editing_target, Some(EditableField::Hsl(i)) if i == idx),
            );
        }

        // ── Footer hints ────────────────────────────────────────────────────
        let key = |s: &'static str| {
            Span::styled(s, Style::new().fg(t.accent_fg).bg(t.accent_bg).add_modifier(Modifier::BOLD))
        };
        let lbl = |s: &'static str| Span::styled(s, Style::new().fg(t.muted));
        Paragraph::new(Line::from(vec![
            key(" Tab "),
            lbl(" focus "),
            key(" M "),
            lbl(" switch "),
            key(" Enter "),
            lbl(" edit "),
            key(" # "),
            lbl(" hex "),
            key(" Esc "),
            lbl(" cancel "),
        ]))
        .style(Style::new().bg(t.bg))
        .render(footer, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorEditor;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn rendered_rows() -> Vec<String> {
        let mut editor = ColorEditor::from_rgb(0x89, 0xb4, 0xfa);
        let mut terminal = Terminal::new(TestBackend::new(76, 24)).unwrap();
        terminal
            .draw(|f| f.render_stateful_widget(ColorPicker::new(), f.area(), &mut editor))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect()
    }

    #[test]
    fn footer_hints_are_one_keyboard_row() {
        let rows = rendered_rows();
        // "cancel" is unique to the footer hint strip.
        let hint_rows: Vec<&String> = rows.iter().filter(|r| r.contains("cancel")).collect();
        assert_eq!(hint_rows.len(), 1, "hints must occupy exactly one row");
        let hints = hint_rows[0];
        assert!(hints.contains("Tab") && hints.contains("focus"));
        assert!(hints.contains("hex") && hints.contains("Esc"));
        // The mouse chip was removed — keyboard hints only.
        assert!(rows.iter().all(|r| !r.contains("Mouse")));
    }
}
