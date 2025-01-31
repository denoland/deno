// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::io;
use std::io::Write;

use crossterm::cursor;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::style;
use crossterm::style::Stylize;
use crossterm::terminal;
use crossterm::ExecutableCommand;
use crossterm::QueueableCommand;
use deno_core::anyhow;
use deno_core::anyhow::Context;

use super::super::deps::DepLocation;
use crate::tools::registry::pm::deps::DepKind;

#[derive(Debug)]
pub struct PackageInfo {
  pub location: DepLocation,
  pub current_version: String,
  pub new_version: String,
  pub name: String,

  pub kind: DepKind,
}

#[derive(Debug)]
struct State {
  packages: Vec<PackageInfo>,
  currently_selected: usize,
  checked: HashSet<usize>,

  name_width: usize,
  current_width: usize,
  start_row: u16,
}

impl State {
  fn new(packages: Vec<PackageInfo>) -> anyhow::Result<Self> {
    let name_width = packages
      .iter()
      .map(|p| p.name.len() + p.kind.scheme().len() + 1)
      .max()
      .unwrap_or_default();
    let current_width = packages
      .iter()
      .map(|p| p.current_version.len())
      .max()
      .unwrap_or_default();

    let mut packages = packages;
    packages
      .sort_by(|a, b| a.location.file_path().cmp(&b.location.file_path()));

    Ok(Self {
      packages,
      currently_selected: 0,
      checked: HashSet::new(),

      name_width,
      current_width,
      start_row: cursor::position()?.1,
    })
  }

  fn render<W: std::io::Write>(&self, out: &mut W) -> anyhow::Result<()> {
    use cursor::MoveTo;
    use style::Print;
    use style::PrintStyledContent;

    crossterm::queue!(
      out,
      MoveTo(0, self.start_row),
      terminal::Clear(terminal::ClearType::FromCursorDown),
      PrintStyledContent("?".blue()),
      Print(" Select which packages to update (<space> to select, ↑/↓/j/k to navigate, enter to accept, <Ctrl-c> to cancel)")
    )?;

    let base = self.start_row + 1;

    for (i, package) in self.packages.iter().enumerate() {
      if self.currently_selected == i {
        crossterm::queue!(
          out,
          MoveTo(0, base + (self.currently_selected as u16)),
          PrintStyledContent("❯".blue()),
          Print(' '),
        )?;
      }
      let checked = self.checked.contains(&i);
      let selector = if checked { "●" } else { "○" };
      crossterm::queue!(
        out,
        MoveTo(2, base + (i as u16)),
        Print(selector),
        Print(" "),
      )?;

      if self.currently_selected == i {
        out.queue(style::SetStyle(
          style::ContentStyle::new().on_black().white().bold(),
        ))?;
      }
      let want = &package.new_version;
      let new_version_highlight =
        highlight_new_version(&package.current_version, want)?;
      // let style = style::PrintStyledContent()
      crossterm::queue!(
        out,
        Print(format!(
          "{:<name_width$} {:<current_width$} -> {}",
          format!(
            "{}{}{}",
            deno_terminal::colors::gray(package.kind.scheme()),
            deno_terminal::colors::gray(":"),
            package.name
          ),
          package.current_version,
          new_version_highlight,
          name_width = self.name_width + 2,
          current_width = self.current_width
        )),
      )?;
      // out.queue(Print(&package.package.name))?;
      if self.currently_selected == i {
        out.queue(style::ResetColor)?;
      }
    }

    out.queue(MoveTo(0, base + self.packages.len() as u16))?;

    out.flush()?;

    Ok(())
  }
}

enum VersionDifference {
  Major,
  Minor,
  Patch,
}

struct VersionParts {
  major: u64,
  minor: u64,
  patch: u64,
  pre: Option<String>,
}

impl VersionParts {
  fn parse(s: &str) -> Result<VersionParts, anyhow::Error> {
    let mut parts = s.splitn(3, '.');
    let major = parts
      .next()
      .ok_or_else(|| anyhow::anyhow!("expected major version"))?
      .parse()?;
    let minor = parts
      .next()
      .ok_or_else(|| anyhow::anyhow!("expected minor version"))?
      .parse()?;
    let patch = parts
      .next()
      .ok_or_else(|| anyhow::anyhow!("expected patch version"))?;
    let (patch, pre) = if patch.contains('-') {
      let (patch, pre) = patch.split_once('-').unwrap();
      (patch, Some(pre.into()))
    } else {
      (patch, None)
    };
    let patch = patch.parse()?;
    let pre = pre.clone();
    Ok(Self {
      patch,
      pre,
      minor,
      major,
    })
  }
}

fn version_diff(a: &VersionParts, b: &VersionParts) -> VersionDifference {
  if a.major != b.major {
    VersionDifference::Major
  } else if a.minor != b.minor {
    VersionDifference::Minor
  } else {
    VersionDifference::Patch
  }
}

fn highlight_new_version(
  current: &str,
  new: &str,
) -> Result<String, anyhow::Error> {
  let current_parts = VersionParts::parse(current)
    .with_context(|| format!("parsing current version: {current}"))?;
  let new_parts = VersionParts::parse(new)
    .with_context(|| format!("parsing new version: {new}"))?;
  let diff = version_diff(&current_parts, &new_parts);

  Ok(match diff {
    VersionDifference::Major => format!(
      "{}.{}.{}{}",
      style::style(new_parts.major).red().bold(),
      style::style(new_parts.minor).red(),
      style::style(new_parts.patch).red(),
      new_parts
        .pre
        .map(|pre| pre.red().to_string())
        .unwrap_or_default()
    ),
    VersionDifference::Minor => format!(
      "{}.{}.{}{}",
      new_parts.major,
      style::style(new_parts.minor).yellow().bold(),
      style::style(new_parts.patch).yellow(),
      new_parts
        .pre
        .map(|pre| pre.yellow().to_string())
        .unwrap_or_default()
    ),
    VersionDifference::Patch => format!(
      "{}.{}.{}{}",
      new_parts.major,
      new_parts.minor,
      style::style(new_parts.patch).green().bold(),
      new_parts
        .pre
        .map(|pre| pre.green().to_string())
        .unwrap_or_default()
    ),
  })
}

struct RawMode {
  needs_disable: bool,
}

impl RawMode {
  fn enable() -> io::Result<Self> {
    terminal::enable_raw_mode()?;
    Ok(Self {
      needs_disable: true,
    })
  }
  fn disable(mut self) -> io::Result<()> {
    self.needs_disable = false;
    terminal::disable_raw_mode()
  }
}

impl Drop for RawMode {
  fn drop(&mut self) {
    if self.needs_disable {
      let _ = terminal::disable_raw_mode();
    }
  }
}

pub fn select_interactive(
  packages: Vec<PackageInfo>,
) -> anyhow::Result<Option<HashSet<usize>>> {
  let mut stdout = io::stdout();
  let raw_mode = RawMode::enable()?;

  let (_, rows) = terminal::size()?;

  let (_, start_row) = cursor::position().unwrap_or_default();
  if rows - start_row < (packages.len() + 2) as u16 {
    let pad = ((packages.len() + 2) as u16) - (rows - start_row);

    stdout.execute(terminal::ScrollUp(pad))?;
    stdout.execute(cursor::MoveUp(pad))?;
  }

  let mut state = State::new(packages)?;
  stdout.execute(cursor::Hide)?;

  state.render(&mut stdout)?;

  let mut do_it = false;
  loop {
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
          state.currently_selected = if state.currently_selected == 0 {
            state.packages.len() - 1
          } else {
            state.currently_selected - 1
          };
        }
        (KeyCode::Down | KeyCode::Char('j'), KeyModifiers::NONE) => {
          state.currently_selected =
            (state.currently_selected + 1) % state.packages.len()
        }
        (KeyCode::Char(' '), _) => {
          if !state.checked.insert(state.currently_selected) {
            state.checked.remove(&state.currently_selected);
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
    state.render(&mut stdout)?;
  }

  crossterm::queue!(
    &mut stdout,
    cursor::MoveTo(0, state.start_row),
    terminal::Clear(terminal::ClearType::FromCursorDown),
    cursor::Show,
  )?;
  stdout.flush()?;

  raw_mode.disable()?;

  if do_it {
    Ok(Some(state.checked.into_iter().collect()))
  } else {
    Ok(None)
  }
}
