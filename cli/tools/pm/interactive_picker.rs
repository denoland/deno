// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::io;

use console_static_text::TextItem;
use crossterm::ExecutableCommand;
use crossterm::cursor;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal;
use deno_core::anyhow;
use deno_terminal::colors;
use unicode_width::UnicodeWidthStr;

use crate::util::console::HideCursorGuard;
use crate::util::console::RawMode;
use crate::util::console::new_console_static_text;

pub fn select_items<T, TRender>(
  instructions_line: &str,
  items: &[T],
  initial_checked: HashSet<usize>,
  mut render_item: TRender,
) -> anyhow::Result<Option<HashSet<usize>>>
where
  TRender: FnMut(usize, bool, bool, &T) -> anyhow::Result<TextItem<'static>>,
{
  if items.is_empty() {
    return Ok(Some(HashSet::new()));
  }

  let mut stderr = io::stderr();

  let raw_mode = RawMode::enable()?;
  let mut static_text = new_console_static_text();
  static_text.keep_cursor_zero_column(true);

  let (_, start_row) = cursor::position().unwrap_or_default();
  let (_, rows) = terminal::size()?;
  if rows - start_row < (items.len() + 2) as u16 {
    let pad = ((items.len() + 2) as u16) - (rows - start_row);
    stderr.execute(terminal::ScrollUp(pad.min(rows)))?;
    stderr.execute(cursor::MoveUp(pad.min(rows)))?;
  }

  let mut currently_selected = 0;
  let mut checked = initial_checked;
  let hide_cursor_guard = HideCursorGuard::hide()?;

  let instructions_width = format!("? {}", instructions_line).width();

  let mut do_it = false;
  let mut scroll_offset = 0;
  loop {
    let mut rendered_items = Vec::with_capacity(items.len() + 1);

    rendered_items.push(TextItem::new_owned(format!(
      "{} {}",
      colors::intense_blue("?"),
      instructions_line
    )));

    for (idx, item) in items.iter().enumerate() {
      rendered_items.push(render_item(
        idx,
        idx == currently_selected,
        checked.contains(&idx),
        item,
      )?);
    }

    let size = static_text.console_size();
    let first_line_rows = size
      .cols
      .map(|cols| (instructions_width / cols as usize) + 1)
      .unwrap_or(1);
    if let Some(rows) = size.rows
      && rendered_items.len() + first_line_rows >= rows as usize
    {
      let adj = if scroll_offset == 0 {
        first_line_rows.saturating_sub(1)
      } else {
        0
      };
      if currently_selected < scroll_offset {
        scroll_offset = currently_selected;
      } else if currently_selected + 1
        >= scroll_offset + (rows as usize).saturating_sub(adj)
      {
        scroll_offset =
          (currently_selected + 1).saturating_sub(rows as usize) + 1;
      }
      let adj = if scroll_offset == 0 {
        first_line_rows.saturating_sub(1)
      } else {
        0
      };
      let mut new_items = Vec::with_capacity(rows as usize);

      scroll_offset = scroll_offset.clamp(0, rendered_items.len() - 1);
      new_items.extend(
        rendered_items.drain(
          scroll_offset
            ..(scroll_offset + (rows as usize).saturating_sub(adj))
              .min(rendered_items.len()),
        ),
      );
      rendered_items = new_items;
    }
    static_text.eprint_items(rendered_items.iter());

    let event = crossterm::event::read()?;
    #[allow(clippy::single_match)]
    match event {
      crossterm::event::Event::Key(KeyEvent {
        kind: KeyEventKind::Press,
        code,
        modifiers,
        ..
      }) => match (code, modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,
        (KeyCode::Up | KeyCode::Char('k'), KeyModifiers::NONE) => {
          currently_selected = if currently_selected == 0 {
            items.len() - 1
          } else {
            currently_selected - 1
          };
        }
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
          currently_selected = (currently_selected + 1) % items.len();
        }
        (KeyCode::Char(' '), _) => {
          if !checked.insert(currently_selected) {
            checked.remove(&currently_selected);
          }
        }
        (KeyCode::Char('a'), _) => {
          if (0..items.len()).all(|idx| checked.contains(&idx)) {
            checked.clear();
          } else {
            checked.extend(0..items.len());
          }
        }
        (KeyCode::Char('i'), _) => {
          for idx in 0..items.len() {
            if checked.contains(&idx) {
              checked.remove(&idx);
            } else {
              checked.insert(idx);
            }
          }
        }
        (KeyCode::Enter, _) => {
          do_it = true;
          break;
        }
        _ => {}
      },
      _ => {}
    }
  }

  static_text.eprint_clear();

  hide_cursor_guard.show()?;
  raw_mode.disable()?;

  if do_it { Ok(Some(checked)) } else { Ok(None) }
}
