// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io;

use console_static_text::ConsoleSize;
use console_static_text::TextItem;
use crossterm::cursor;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal;
use crossterm::ExecutableCommand;
use deno_core::anyhow;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_terminal::colors;

use crate::tools::registry::pm::deps::DepKind;

#[derive(Debug)]
pub struct PackageInfo {
  pub current_version: Option<Version>,
  pub new_version: VersionReq,
  pub name: String,
  pub kind: DepKind,
}

#[derive(Debug)]
struct FormattedPackageInfo {
  current_version_string: Option<String>,
  new_version_highlighted: String,
  formatted_name: String,
  formatted_name_len: usize,
}

#[derive(Debug)]
struct State {
  packages: Vec<FormattedPackageInfo>,
  currently_selected: usize,
  checked: HashSet<usize>,

  name_width: usize,
  current_width: usize,
  // start_row: u16,
}

impl From<PackageInfo> for FormattedPackageInfo {
  fn from(package: PackageInfo) -> Self {
    let new_version_string =
      package.new_version.version_text().trim_start_matches('^');

    let new_version_highlighted =
      if let (Some(current_version), Ok(new_version)) = (
        &package.current_version,
        Version::parse_standard(new_version_string),
      ) {
        highlight_new_version(current_version, &new_version)
      } else {
        new_version_string.to_string()
      };
    FormattedPackageInfo {
      current_version_string: package
        .current_version
        .as_ref()
        .map(|v| v.to_string()),
      new_version_highlighted,
      formatted_name: format!(
        "{}{}",
        colors::gray(format!("{}:", package.kind.scheme())),
        package.name
      ),
      formatted_name_len: package.kind.scheme().len() + 1 + package.name.len(),
    }
  }
}

impl State {
  fn new(packages: Vec<PackageInfo>) -> anyhow::Result<Self> {
    let packages: Vec<_> = packages
      .into_iter()
      .map(FormattedPackageInfo::from)
      .collect();
    let name_width = packages
      .iter()
      .map(|p| p.formatted_name_len)
      .max()
      .unwrap_or_default();
    let current_width = packages
      .iter()
      .map(|p| {
        p.current_version_string
          .as_ref()
          .map(|s| s.len())
          .unwrap_or_default()
      })
      .max()
      .unwrap_or_default();

    Ok(Self {
      packages,
      currently_selected: 0,
      checked: HashSet::new(),

      name_width,
      current_width,
    })
  }

  fn render(&self) -> anyhow::Result<Vec<TextItem>> {
    let mut items = Vec::with_capacity(self.packages.len() + 1);

    items.push(TextItem::new_owned(format!(
      "{} Select which packages to update (<space> to select, ↑/↓/j/k to navigate, enter to accept, <Ctrl-c> to cancel)",
      colors::intense_blue("?")
    )));

    for (i, package) in self.packages.iter().enumerate() {
      let mut line = String::new();
      let f = &mut line;

      let checked = self.checked.contains(&i);
      write!(
        f,
        "{} {} ",
        if self.currently_selected == i {
          colors::intense_blue("❯").to_string()
        } else {
          " ".to_string()
        },
        if checked { "●" } else { "○" }
      )?;

      let name_pad =
        " ".repeat(self.name_width + 2 - package.formatted_name_len);
      write!(
        f,
        "{formatted_name}{name_pad} {:<current_width$} -> {}",
        package
          .current_version_string
          .as_deref()
          .unwrap_or_default(),
        &package.new_version_highlighted,
        name_pad = name_pad,
        formatted_name = package.formatted_name,
        current_width = self.current_width
      )?;

      items.push(TextItem::with_hanging_indent_owned(line, 1));
    }

    Ok(items)
  }
}

enum VersionDifference {
  Major,
  Minor,
  Patch,
}

fn version_diff(a: &Version, b: &Version) -> VersionDifference {
  if a.major != b.major {
    VersionDifference::Major
  } else if a.minor != b.minor {
    VersionDifference::Minor
  } else {
    VersionDifference::Patch
  }
}

fn highlight_new_version(current: &Version, new: &Version) -> String {
  let diff = version_diff(current, new);

  let new_pre = if new.pre.is_empty() {
    String::new()
  } else {
    let mut s = String::new();
    s.push('-');
    for p in &new.pre {
      s.push_str(p);
    }
    s
  };

  match diff {
    VersionDifference::Major => format!(
      "{}.{}.{}{}",
      colors::red_bold(new.major),
      colors::red(new.minor),
      colors::red(new.patch),
      colors::red(new_pre)
    ),
    VersionDifference::Minor => format!(
      "{}.{}.{}{}",
      new.major,
      colors::yellow_bold(new.minor),
      colors::yellow(new.patch),
      colors::yellow(new_pre)
    ),
    VersionDifference::Patch => format!(
      "{}.{}.{}{}",
      new.major,
      new.minor,
      colors::green_bold(new.patch),
      colors::green(new_pre)
    ),
  }
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
  let mut stderr = io::stderr();

  let raw_mode = RawMode::enable()?;
  let mut static_text =
    console_static_text::ConsoleStaticText::new(move || {
      if let Ok((cols, rows)) = terminal::size() {
        ConsoleSize {
          cols: Some(cols),
          rows: Some(rows),
        }
      } else {
        ConsoleSize {
          cols: None,
          rows: None,
        }
      }
    });
  static_text.keep_cursor_zero_column(true);

  let (_, start_row) = cursor::position().unwrap_or_default();
  let (_, rows) = terminal::size()?;
  if rows - start_row < (packages.len() + 2) as u16 {
    let pad = ((packages.len() + 2) as u16) - (rows - start_row);

    stderr.execute(terminal::ScrollUp(pad))?;
    stderr.execute(cursor::MoveUp(pad))?;
  }

  let mut state = State::new(packages)?;
  stderr.execute(cursor::Hide)?;

  let mut do_it = false;
  loop {
    let items = state.render()?;
    static_text.eprint_items(items.iter());

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
  }

  static_text.eprint_clear();

  crossterm::execute!(&mut stderr, cursor::Show)?;

  raw_mode.disable()?;

  if do_it {
    Ok(Some(state.checked.into_iter().collect()))
  } else {
    Ok(None)
  }
}
