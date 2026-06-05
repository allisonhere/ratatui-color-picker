//! Minimal interactive demo: `cargo run --example demo`.
//!
//! Tab/Shift-Tab move focus, M toggles RGB/HSL, arrows nudge the focused control,
//! Enter edits the focused field (then type + Enter to commit), `#` jumps to hex,
//! Esc cancels an edit, and `q` quits — printing the picked color.

use std::io;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Frame;
use ratatui_color_picker::{ColorEditor, ColorPicker, ColorPickerFocus, RgbColor};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut editor = ColorEditor::from_rgb(0x89, 0xb4, 0xfa);
    let original = editor.to_rgb();
    let result = run(&mut terminal, &mut editor, original);
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
        terminal.draw(|f| draw(f, editor, original))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };

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

        let step = if key.modifiers.contains(KeyModifiers::SHIFT) { 10.0 } else { 1.0 };
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
            KeyCode::Up => {
                editor.adjust_focused_numeric(step);
            }
            KeyCode::Down => {
                editor.adjust_focused_numeric(-step);
            }
            KeyCode::Right => match editor.focus {
                ColorPickerFocus::RgbSlider(_) => editor.move_rgb_slider_focus(false),
                ColorPickerFocus::HslField => editor.nudge_hsl_field(5.0, 0.0),
                _ => editor.focus_next(false),
            },
            KeyCode::Left => match editor.focus {
                ColorPickerFocus::RgbSlider(_) => editor.move_rgb_slider_focus(true),
                ColorPickerFocus::HslField => editor.nudge_hsl_field(-5.0, 0.0),
                _ => editor.focus_next(true),
            },
            _ => {}
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, editor: &mut ColorEditor, original: RgbColor) {
    let area = centered(f.area(), 76, 24);
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
