// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Write as _;

use console_static_text::TextItem;
use deno_core::anyhow;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_terminal::colors;

use crate::tools::pm::deps::DepId;
use crate::tools::pm::deps::DepKind;
use crate::tools::pm::interactive_picker;

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
  name_width: usize,
  current_width: usize,
}

impl From<PackageInfo> for FormattedPackageInfo {
  fn from(package: PackageInfo) -> Self {
    let new_version_string =
      package.new_version.version_text().trim_start_matches('^');

    let new_version_highlighted = match (
      &package.current_version,
      Version::parse_standard(new_version_string),
    ) {
      (Some(current_version), Ok(new_version)) => {
        highlight_new_version(current_version, &new_version)
      }
      _ => new_version_string.to_string(),
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
      name_width,
      current_width,
    })
  }

  fn instructions_line() -> &'static str {
    "Select which packages to update (<space> to select, ↑/↓/j/k to navigate, a to select all, i to invert selection, enter to accept, <Ctrl-c> to cancel)"
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

fn render_package(
  package: &FormattedPackageInfo,
  name_width: usize,
  current_width: usize,
  is_selected: bool,
  is_checked: bool,
) -> anyhow::Result<TextItem<'static>> {
  let mut line = String::new();
  let f = &mut line;

  write!(
    f,
    "{} {} ",
    if is_selected {
      colors::intense_blue("❯").to_string()
    } else {
      " ".to_string()
    },
    if is_checked { "●" } else { "○" }
  )?;

  let name_pad = " ".repeat(name_width + 2 - package.formatted_name_len);
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
    current_width = current_width
  )?;

  Ok(TextItem::with_hanging_indent_owned(line, 1))
}

pub fn select_interactive(
  packages: Vec<PackageInfo>,
) -> anyhow::Result<Option<HashSet<DepId>>> {
  let state = State::new(packages)?;
  let name_width = state.name_width;
  let current_width = state.current_width;
  let packages = state.packages;

  let selected = interactive_picker::select_items(
    State::instructions_line(),
    &packages,
    HashSet::new(),
    |_idx, is_selected, is_checked, package| {
      render_package(
        package,
        name_width,
        current_width,
        is_selected,
        is_checked,
      )
    },
  )?;

  Ok(selected.map(|indices| {
    indices
      .into_iter()
      .flat_map(|idx| &packages[idx].dep_ids)
      .copied()
      .collect()
  }))
}
