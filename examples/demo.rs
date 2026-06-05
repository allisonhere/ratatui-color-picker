//! Minimal interactive demo: `cargo run --example demo`.
//!
//! Keyboard: Tab/Shift-Tab move focus, M toggles RGB/HSL, arrows nudge the focused
//! control (in the HSL field, ←→ change hue and ↑↓ change saturation), Enter edits the
//! focused field (then type + Enter to commit), `#` jumps to hex, Esc cancels, `q` quits.
//!
//! Mouse: click a control to focus it, click/drag inside the HSL field or lightness
//! slider to pick, click the rgb/hsl mode pills to switch.

use std::io;

use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
    MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui_color_picker::{
    picker_layout, ColorDragTarget, ColorEditor, ColorPicker, ColorPickerFocus, ColorPickerMode,
    PickerRects, RgbColor,
};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut editor = ColorEditor::from_rgb(0x89, 0xb4, 0xfa);
    let original = editor.to_rgb();
    let result = run(&mut terminal, &mut editor, original);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result?;
    println!("picked {}", editor.hex());
    Ok(())
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    editor: &mut ColorEditor,
    original: RgbColor,
) -> io::Result<()> {
    loop {
        // Compute the picker area once so drawing and mouse hit-testing agree.
        let (w, h) = ratatui::crossterm::terminal::size()?;
        let area = centered(Rect::new(0, 0, w, h), 76, 24);

        terminal.draw(|f| draw(f, editor, original, area))?;

        match event::read()? {
            Event::Key(key) => {
                if editor.is_editing_text() {
                    match key.code {
                        KeyCode::Esc => editor.cancel_text_edit(),
                        KeyCode::Enter => {
                            editor.commit_text_edit();
                        }
                        KeyCode::Backspace => {
                            editor.pop_input_char();
                        }
                        KeyCode::Char(c) => {
                            editor.push_input_char(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                let step = if shift { 10.0 } else { 1.0 };
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => editor.focus_next(false),
                    KeyCode::BackTab => editor.focus_next(true),
                    KeyCode::Char('m') | KeyCode::Char('M') => editor.toggle_mode(),
                    KeyCode::Char('#') => editor.start_hex_input(),
                    KeyCode::Enter => match editor.focus {
                        ColorPickerFocus::HexField
                        | ColorPickerFocus::RgbField(_)
                        | ColorPickerFocus::HslFieldValue(_) => editor.start_editing_focused(),
                        ColorPickerFocus::ModeToggle => editor.toggle_mode(),
                        _ => {}
                    },
                    KeyCode::Up => match editor.focus {
                        ColorPickerFocus::HslField => {
                            editor.nudge_hsl_field(0.0, if shift { 10.0 } else { 2.0 })
                        }
                        _ => {
                            editor.adjust_focused_numeric(step);
                        }
                    },
                    KeyCode::Down => match editor.focus {
                        ColorPickerFocus::HslField => {
                            editor.nudge_hsl_field(0.0, if shift { -10.0 } else { -2.0 })
                        }
                        _ => {
                            editor.adjust_focused_numeric(-step);
                        }
                    },
                    KeyCode::Right => match editor.focus {
                        ColorPickerFocus::RgbSlider(_) => editor.move_rgb_slider_focus(false),
                        ColorPickerFocus::HslField => {
                            editor.nudge_hsl_field(if shift { 20.0 } else { 5.0 }, 0.0)
                        }
                        _ => editor.focus_next(false),
                    },
                    KeyCode::Left => match editor.focus {
                        ColorPickerFocus::RgbSlider(_) => editor.move_rgb_slider_focus(true),
                        ColorPickerFocus::HslField => {
                            editor.nudge_hsl_field(if shift { -20.0 } else { -5.0 }, 0.0)
                        }
                        _ => editor.focus_next(true),
                    },
                    _ => {}
                }
            }
            Event::Mouse(m) => {
                let rects = picker_layout(area, editor.mode);
                match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(focus) = editor.focus_for_point(&rects, m.column, m.row) {
                            match focus {
                                ColorPickerFocus::ModeToggle => {
                                    let mid = rects.mode_switch.x + rects.mode_switch.width / 2;
                                    let clicked = if m.column >= mid {
                                        ColorPickerMode::HslField
                                    } else {
                                        ColorPickerMode::RgbSliders
                                    };
                                    if editor.mode != clicked {
                                        editor.toggle_mode();
                                    }
                                }
                                ColorPickerFocus::HslField => {
                                    editor.set_drag_target(Some(ColorDragTarget::HslField));
                                    update_drag(editor, ColorDragTarget::HslField, m.column, m.row, &rects);
                                }
                                ColorPickerFocus::LightnessSlider => {
                                    editor.set_drag_target(Some(ColorDragTarget::LightnessSlider));
                                    update_drag(editor, ColorDragTarget::LightnessSlider, m.column, m.row, &rects);
                                }
                                other => editor.set_focus(other),
                            }
                        }
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if let Some(target) = editor.drag_target {
                            update_drag(editor, target, m.column, m.row, &rects);
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => editor.set_drag_target(None),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Map a mouse position to the HSL field / lightness slider, mirroring how the picker
/// lays out those controls inside `main_view` / `aux_slider`.
fn update_drag(
    editor: &mut ColorEditor,
    target: ColorDragTarget,
    column: u16,
    row: u16,
    rects: &PickerRects,
) {
    match target {
        ColorDragTarget::HslField => {
            if rects.main_view.width > 0 && rects.main_view.height > 0 {
                let x = column
                    .saturating_sub(rects.main_view.x)
                    .min(rects.main_view.width.saturating_sub(1));
                let y = row
                    .saturating_sub(rects.main_view.y)
                    .min(rects.main_view.height.saturating_sub(1));
                let x_frac = x as f32 / rects.main_view.width.saturating_sub(1).max(1) as f32;
                let y_frac = y as f32 / rects.main_view.height.saturating_sub(1).max(1) as f32;
                editor.update_from_hsl_field(x_frac, y_frac);
            }
        }
        ColorDragTarget::LightnessSlider => {
            if rects.aux_slider.height > 0 {
                let y = row
                    .saturating_sub(rects.aux_slider.y)
                    .min(rects.aux_slider.height.saturating_sub(1));
                let y_frac = y as f32 / rects.aux_slider.height.saturating_sub(1).max(1) as f32;
                editor.update_lightness_from_frac(y_frac);
            }
        }
    }
}

fn draw(f: &mut Frame, editor: &mut ColorEditor, original: RgbColor, area: Rect) {
    f.render_stateful_widget(ColorPicker::new().original(Some(original)), area, editor);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}
