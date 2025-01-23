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

use super::super::deps::DepLocation;
use super::OutdatedPackage;

#[derive(Debug)]
struct PackageInfo {
  location: DepLocation,
  package: OutdatedPackage,
}

#[derive(Debug)]
struct State {
  packages: Vec<PackageInfo>,
  currently_selected: usize,
  checked: HashSet<usize>,

  name_width: usize,
  current_width: usize,
}

impl State {
  fn new(packages: Vec<PackageInfo>) -> Self {
    let name_width = packages
      .iter()
      .map(|p| p.package.name.len())
      .max()
      .unwrap_or_default();
    let current_width = packages
      .iter()
      .map(|p| p.package.current.len())
      .max()
      .unwrap_or_default();

    let mut packages = packages;
    packages
      .sort_by(|a, b| a.location.file_path().cmp(&b.location.file_path()));

    Self {
      packages,
      currently_selected: 0,
      checked: HashSet::new(),

      name_width,
      current_width,
    }
  }

  fn render<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()> {
    use cursor::MoveTo;
    use style::Print;
    use style::PrintStyledContent;

    crossterm::queue!(
      out,
      terminal::Clear(terminal::ClearType::All),
      MoveTo(0, 0),
      PrintStyledContent("?".blue()),
    )?;

    let base = 1;

    for (i, package) in self.packages.iter().enumerate() {
      if self.currently_selected == i {
        crossterm::queue!(
          out,
          MoveTo(1, base + (self.currently_selected as u16)),
          PrintStyledContent("❯".blue()),
          Print(' '),
        )?;
      }
      let checked = self.checked.contains(&i);
      let selector = if checked { "●" } else { "○" };
      crossterm::queue!(
        out,
        MoveTo(3, base + (i as u16)),
        Print(selector),
        Print(" "),
      )?;

      if self.currently_selected == i {
        out.queue(style::SetStyle(
          style::ContentStyle::new().on_black().white().bold(),
        ))?;
      }
      let want = &package.package.latest;
      crossterm::queue!(
        out,
        Print(format!(
          "{:<name_width$}{:<current_width$}   ->   {}",
          package.package.name,
          package.package.current,
          highlight_new_version(&package.package.current, want),
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
  fn parse(s: &str) -> VersionParts {
    let mut parts = s.splitn(3, '.');
    let major = parts.next().unwrap().parse().unwrap();
    let minor = parts.next().unwrap().parse().unwrap();
    let patch = parts.next().unwrap();
    let (patch, pre) = if patch.contains('-') {
      let (patch, pre) = patch.split_once('-').unwrap();
      (patch, Some(pre.into()))
    } else {
      (patch, None)
    };
    let patch = patch.parse().unwrap();
    let pre = pre.clone();
    Self {
      patch,
      pre,
      minor,
      major,
    }
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

fn highlight_new_version(current: &str, new: &str) -> String {
  let current_parts = VersionParts::parse(current);
  let new_parts = VersionParts::parse(new);
  let diff = version_diff(&current_parts, &new_parts);

  match diff {
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
  }
}

fn interactive() -> io::Result<()> {
  let mut stdout = io::stdout();
  terminal::enable_raw_mode()?;

  let mut state = State::new(todo!());

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
    terminal::Clear(terminal::ClearType::All),
    cursor::Show,
    cursor::MoveTo(0, 0),
  )?;
  stdout.flush()?;

  terminal::disable_raw_mode()?;

  if do_it {
    println!("doing the thing... {state:?}");
  }

  Ok(())
}
