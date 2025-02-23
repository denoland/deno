// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
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
use unicode_width::UnicodeWidthStr;

use crate::tools::pm::deps::DepId;
use crate::tools::pm::deps::DepKind;

#[derive(Debug)]
pub struct PackageInfo {
  pub id: DepId,
  pub current_version: Option<Version>,
  pub new_version: VersionReq,
  pub name: String,
  pub kind: DepKind,
}

#[derive(Debug)]
struct FormattedPackageInfo {
  dep_ids: Vec<DepId>,
  current_version_string: Option<String>,
  new_version_highlighted: String,
  formatted_name: String,
  formatted_name_len: usize,
  name: String,
}

#[derive(Debug)]
struct State {
  packages: Vec<FormattedPackageInfo>,
  currently_selected: usize,
  checked: HashSet<usize>,

  name_width: usize,
  current_width: usize,
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
      dep_ids: vec![package.id],
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
      name: package.name,
    }
  }
}

impl State {
  fn new(packages: Vec<PackageInfo>) -> anyhow::Result<Self> {
    let mut deduped_packages: HashMap<
      (String, Option<Version>, VersionReq),
      FormattedPackageInfo,
    > = HashMap::with_capacity(packages.len());
    for package in packages {
      match deduped_packages.entry((
        package.name.clone(),
        package.current_version.clone(),
        package.new_version.clone(),
      )) {
        std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
          occupied_entry.get_mut().dep_ids.push(package.id)
        }
        std::collections::hash_map::Entry::Vacant(vacant_entry) => {
          vacant_entry.insert(FormattedPackageInfo::from(package));
        }
      }
    }

    let mut packages: Vec<_> = deduped_packages.into_values().collect();
    packages.sort_by(|a, b| a.name.cmp(&b.name));
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

  fn instructions_line() -> &'static str {
    "Select which packages to update (<space> to select, ↑/↓/j/k to navigate, a to select all, i to invert selection, enter to accept, <Ctrl-c> to cancel)"
  }

  fn render(&self) -> anyhow::Result<Vec<TextItem>> {
    let mut items = Vec::with_capacity(self.packages.len() + 1);

    items.push(TextItem::new_owned(format!(
      "{} {}",
      colors::intense_blue("?"),
      Self::instructions_line()
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
  Prerelease,
}

fn version_diff(a: &Version, b: &Version) -> VersionDifference {
  if a.major != b.major {
    VersionDifference::Major
  } else if a.minor != b.minor {
    VersionDifference::Minor
  } else if a.patch != b.patch {
    VersionDifference::Patch
  } else {
    VersionDifference::Prerelease
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
      colors::red_bold(new.minor),
      colors::red_bold(new.patch),
      colors::red_bold(new_pre)
    ),
    VersionDifference::Minor => format!(
      "{}.{}.{}{}",
      new.major,
      colors::yellow_bold(new.minor),
      colors::yellow_bold(new.patch),
      colors::yellow_bold(new_pre)
    ),
    VersionDifference::Patch => format!(
      "{}.{}.{}{}",
      new.major,
      new.minor,
      colors::green_bold(new.patch),
      colors::green_bold(new_pre)
    ),
    VersionDifference::Prerelease => format!(
      "{}.{}.{}{}",
      new.major,
      new.minor,
      new.patch,
      colors::red_bold(new_pre)
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
) -> anyhow::Result<Option<HashSet<DepId>>> {
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
    stderr.execute(terminal::ScrollUp(pad.min(rows)))?;
    stderr.execute(cursor::MoveUp(pad.min(rows)))?;
  }

  let mut state = State::new(packages)?;
  stderr.execute(cursor::Hide)?;

  let instructions_width = format!("? {}", State::instructions_line()).width();

  let mut do_it = false;
  let mut scroll_offset = 0;
  loop {
    let mut items = state.render()?;
    let size = static_text.console_size();
    let first_line_rows = size
      .cols
      .map(|cols| (instructions_width / cols as usize) + 1)
      .unwrap_or(1);
    if let Some(rows) = size.rows {
      if items.len() + first_line_rows >= rows as usize {
        let adj = if scroll_offset == 0 {
          first_line_rows.saturating_sub(1)
        } else {
          0
        };
        if state.currently_selected < scroll_offset {
          scroll_offset = state.currently_selected;
        } else if state.currently_selected + 1
          >= scroll_offset + (rows as usize).saturating_sub(adj)
        {
          scroll_offset =
            (state.currently_selected + 1).saturating_sub(rows as usize) + 1;
        }
        let adj = if scroll_offset == 0 {
          first_line_rows.saturating_sub(1)
        } else {
          0
        };
        let mut new_items = Vec::with_capacity(rows as usize);

        scroll_offset = scroll_offset.clamp(0, items.len() - 1);
        new_items.extend(
          items.drain(
            scroll_offset
              ..(scroll_offset + (rows as usize).saturating_sub(adj))
                .min(items.len()),
          ),
        );
        items = new_items;
      }
    }
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
            (state.currently_selected + 1) % state.packages.len();
        }
        (KeyCode::Char(' '), _) => {
          if !state.checked.insert(state.currently_selected) {
            state.checked.remove(&state.currently_selected);
          }
        }
        (KeyCode::Char('a'), _) => {
          if (0..state.packages.len()).all(|idx| state.checked.contains(&idx)) {
            state.checked.clear();
          } else {
            state.checked.extend(0..state.packages.len());
          }
        }
        (KeyCode::Char('i'), _) => {
          for idx in 0..state.packages.len() {
            if state.checked.contains(&idx) {
              state.checked.remove(&idx);
            } else {
              state.checked.insert(idx);
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

  crossterm::execute!(&mut stderr, cursor::Show)?;

  raw_mode.disable()?;

  if do_it {
    Ok(Some(
      state
        .checked
        .into_iter()
        .flat_map(|idx| &state.packages[idx].dep_ids)
        .copied()
        .collect(),
    ))
  } else {
    Ok(None)
  }
}
