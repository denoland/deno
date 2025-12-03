// Copyright 2018-2025 the Deno authors. MIT license.

// Windows shim generation ported from https://github.com/npm/cmd-shim
// Original code licensed under the ISC License:
//
// The ISC License
//
// Copyright (c) npm, Inc. and Contributors
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR
// IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fmt::Write;
use std::io::BufRead;
use std::path::Path;
use std::path::PathBuf;

use deno_npm::NpmPackageExtraInfo;
use deno_npm::NpmPackageId;
use deno_npm::NpmResolutionPackage;
use deno_npm::resolution::NpmResolutionSnapshot;
use sys_traits::FsCreateDirAll;
use sys_traits::FsFileMetadata;
use sys_traits::FsFileSetPermissions;
use sys_traits::FsMetadata;
use sys_traits::FsMetadataValue;
use sys_traits::FsOpen;
use sys_traits::FsReadLink;
use sys_traits::FsRemoveFile;
use sys_traits::FsSymlinkFile;
use sys_traits::FsWrite;

/// Returns the name of the default binary for the given package.
/// This is the package name without the organization (`@org/`), if any.
fn default_bin_name(package: &NpmResolutionPackage) -> &str {
  package
    .id
    .nv
    .name
    .as_str()
    .rsplit_once('/')
    .map(|(_, name)| name)
    .unwrap_or(package.id.nv.name.as_str())
}

pub fn warn_missing_entrypoint(
  bin_name: &str,
  package_path: &Path,
  entrypoint: &Path,
) {
  log::warn!(
    "{} Trying to set up '{}' bin for \"{}\", but the entry point \"{}\" doesn't exist.",
    deno_terminal::colors::yellow("Warning"),
    bin_name,
    package_path.display(),
    entrypoint.display()
  );
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum BinEntriesError {
  #[class(inherit)]
  #[error("Creating '{path}'")]
  Creating {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Setting permissions on '{path}'")]
  Permissions {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error("Can't set up '{name}' bin at {path}")]
  SetUpBin {
    name: String,
    path: PathBuf,
    #[source]
    #[inherit]
    source: Box<Self>,
  },
  #[class(inherit)]
  #[error("Setting permissions on '{path}'")]
  RemoveBinSymlink {
    path: PathBuf,
    #[source]
    #[inherit]
    source: std::io::Error,
  },
  #[class(inherit)]
  #[error(transparent)]
  Io(#[from] std::io::Error),
}

pub struct BinEntries<'a, TSys: SetupBinEntrySys> {
  /// Packages that have colliding bin names
  collisions: HashSet<&'a NpmPackageId>,
  seen_names: HashMap<String, &'a NpmPackageId>,
  /// The bin entries
  entries: Vec<(&'a NpmResolutionPackage, PathBuf, NpmPackageExtraInfo)>,
  sorted: bool,
  sys: &'a TSys,
}

impl<'a, TSys: SetupBinEntrySys> BinEntries<'a, TSys> {
  pub fn new(sys: &'a TSys) -> Self {
    Self {
      collisions: Default::default(),
      seen_names: Default::default(),
      entries: Default::default(),
      sorted: false,
      sys,
    }
  }

  /// Add a new bin entry (package with a bin field)
  pub fn add<'b>(
    &mut self,
    package: &'a NpmResolutionPackage,
    extra: &'b NpmPackageExtraInfo,
    package_path: PathBuf,
  ) {
    self.sorted = false;
    // check for a new collision, if we haven't already
    // found one
    match extra.bin.as_ref().unwrap() {
      deno_npm::registry::NpmPackageVersionBinEntry::String(_) => {
        let bin_name = default_bin_name(package);

        if let Some(other) =
          self.seen_names.insert(bin_name.to_string(), &package.id)
        {
          self.collisions.insert(&package.id);
          self.collisions.insert(other);
        }
      }
      deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
        for name in entries.keys() {
          if let Some(other) =
            self.seen_names.insert(name.to_string(), &package.id)
          {
            self.collisions.insert(&package.id);
            self.collisions.insert(other);
          }
        }
      }
    }

    self.entries.push((package, package_path, extra.clone()));
  }

  fn for_each_entry(
    &mut self,
    snapshot: &NpmResolutionSnapshot,
    mut already_seen: impl FnMut(
      &Path,
      &str, // bin script
    ) -> Result<(), BinEntriesError>,
    mut new: impl FnMut(
      &NpmResolutionPackage,
      &NpmPackageExtraInfo,
      &Path,
      &str, // bin name
      &str, // bin script
    ) -> Result<(), BinEntriesError>,
    mut filter: impl FnMut(&NpmResolutionPackage) -> bool,
  ) -> Result<(), BinEntriesError> {
    if !self.collisions.is_empty() && !self.sorted {
      // walking the dependency tree to find out the depth of each package
      // is sort of expensive, so we only do it if there's a collision
      sort_by_depth(snapshot, &mut self.entries, &mut self.collisions);
      self.sorted = true;
    }

    let mut seen = HashSet::new();

    for (package, package_path, extra) in &self.entries {
      if !filter(package) {
        continue;
      }
      if let Some(bin_entries) = &extra.bin {
        match bin_entries {
          deno_npm::registry::NpmPackageVersionBinEntry::String(script) => {
            let name = default_bin_name(package);
            if !seen.insert(name) {
              already_seen(package_path, script)?;
              // we already set up a bin entry with this name
              continue;
            }
            new(package, extra, package_path, name, script)?;
          }
          deno_npm::registry::NpmPackageVersionBinEntry::Map(entries) => {
            for (name, script) in entries {
              if !seen.insert(name) {
                already_seen(package_path, script)?;
                // we already set up a bin entry with this name
                continue;
              }
              new(package, extra, package_path, name, script)?;
            }
          }
        }
      }
    }

    Ok(())
  }

  /// Collect the bin entries into a vec of (name, script path)
  pub fn collect_bin_files(
    &mut self,
    snapshot: &NpmResolutionSnapshot,
  ) -> Vec<(String, PathBuf)> {
    let mut bins = Vec::new();
    self
      .for_each_entry(
        snapshot,
        |_, _| Ok(()),
        |_, _, package_path, name, script| {
          bins.push((name.to_string(), package_path.join(script)));
          Ok(())
        },
        |_| true,
      )
      .unwrap();
    bins
  }

  fn set_up_entries_filtered(
    mut self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    filter: impl FnMut(&NpmResolutionPackage) -> bool,
    mut handler: impl FnMut(&EntrySetupOutcome<'_>),
  ) -> Result<(), BinEntriesError> {
    if !self.entries.is_empty()
      && !self.sys.fs_exists_no_err(bin_node_modules_dir_path)
    {
      self
        .sys
        .fs_create_dir_all(bin_node_modules_dir_path)
        .map_err(|source| BinEntriesError::Creating {
          path: bin_node_modules_dir_path.to_path_buf(),
          source,
        })?;
    }

    self.for_each_entry(
      snapshot,
      |_package_path, _script| {
        if !sys_traits::impls::is_windows() {
          let path = _package_path.join(_script);
          make_executable_if_exists(self.sys, &path)?;
        }
        Ok(())
      },
      |package, extra, package_path, name, script| {
        let outcome = set_up_bin_entry(
          self.sys,
          package,
          extra,
          name,
          script,
          package_path,
          bin_node_modules_dir_path,
        )?;
        handler(&outcome);
        Ok(())
      },
      filter,
    )?;

    Ok(())
  }

  /// Finish setting up the bin entries, writing the necessary files
  /// to disk.
  pub fn finish(
    self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    handler: impl FnMut(&EntrySetupOutcome<'_>),
  ) -> Result<(), BinEntriesError> {
    self.set_up_entries_filtered(
      snapshot,
      bin_node_modules_dir_path,
      |_| true,
      handler,
    )
  }

  /// Finish setting up the bin entries, writing the necessary files
  /// to disk.
  pub fn finish_only(
    self,
    snapshot: &NpmResolutionSnapshot,
    bin_node_modules_dir_path: &Path,
    handler: impl FnMut(&EntrySetupOutcome<'_>),
    only: &HashSet<&NpmPackageId>,
  ) -> Result<(), BinEntriesError> {
    self.set_up_entries_filtered(
      snapshot,
      bin_node_modules_dir_path,
      |package| only.contains(&package.id),
      handler,
    )
  }
}

// walk the dependency tree to find out the depth of each package
// that has a bin entry, then sort them by depth
fn sort_by_depth(
  snapshot: &NpmResolutionSnapshot,
  bin_entries: &mut [(&NpmResolutionPackage, PathBuf, NpmPackageExtraInfo)],
  collisions: &mut HashSet<&NpmPackageId>,
) {
  enum Entry<'a> {
    Pkg(&'a NpmPackageId),
    IncreaseDepth,
  }

  let mut seen = HashSet::new();
  let mut depths: HashMap<&NpmPackageId, u64> =
    HashMap::with_capacity(collisions.len());

  let mut queue = VecDeque::new();
  queue.extend(snapshot.top_level_packages().map(Entry::Pkg));
  seen.extend(snapshot.top_level_packages());
  queue.push_back(Entry::IncreaseDepth);

  let mut current_depth = 0u64;

  while let Some(entry) = queue.pop_front() {
    if collisions.is_empty() {
      break;
    }
    let id = match entry {
      Entry::Pkg(id) => id,
      Entry::IncreaseDepth => {
        current_depth += 1;
        if queue.is_empty() {
          break;
        }
        queue.push_back(Entry::IncreaseDepth);
        continue;
      }
    };
    if let Some(package) = snapshot.package_from_id(id) {
      if collisions.remove(&package.id) {
        depths.insert(&package.id, current_depth);
      }
      for dep in package.dependencies.values() {
        if seen.insert(dep) {
          queue.push_back(Entry::Pkg(dep));
        }
      }
    }
  }

  bin_entries.sort_by(|(a, _, _), (b, _, _)| {
    depths
      .get(&a.id)
      .unwrap_or(&u64::MAX)
      .cmp(depths.get(&b.id).unwrap_or(&u64::MAX))
      .then_with(|| a.id.nv.cmp(&b.id.nv).reverse())
  });
}

#[sys_traits::auto_impl]
pub trait SetupBinEntrySys:
  FsOpen
  + FsWrite
  + FsSymlinkFile
  + FsRemoveFile
  + FsCreateDirAll
  + FsMetadata
  + FsReadLink
{
}

pub fn set_up_bin_entry<'a>(
  sys: &impl SetupBinEntrySys,
  package: &'a NpmResolutionPackage,
  extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  if sys_traits::impls::is_windows() {
    set_up_bin_shim(
      sys,
      package,
      extra,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )?;
    Ok(EntrySetupOutcome::Success)
  } else {
    symlink_bin_entry(
      sys,
      package,
      extra,
      bin_name,
      bin_script,
      package_path,
      bin_node_modules_dir_path,
    )
  }
}

macro_rules! writeln {
    ($($arg:tt)*) => {
      {
        let _ = std::writeln!($($arg)*);
      }
    };
}

// note: parts of logic and pretty much all of the shims ported from https://github.com/npm/cmd-shim
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Shebang {
  pub program: String,
  pub args: String,
  pub vars: String,
}

fn parse_shebang(s: &str) -> Option<Shebang> {
  let s = s.trim();
  if !s.starts_with("#!") {
    return None;
  }
  let regex = lazy_regex::regex!(
    r"^#!\s*(?:/usr/bin/env\s+(?:-S\s+)?((?:[^ \t=]+=[^ \t=]+\s+)*))?([^ \t\r\n]+)(.*)$"
  );
  let captures = regex.captures(s)?;
  Some(Shebang {
    vars: captures
      .get(1)
      .map(|m| m.as_str().to_string())
      .unwrap_or_default(),
    program: captures.get(2)?.as_str().to_string(),
    args: captures
      .get(3)
      .map(|m| m.as_str().trim().to_string())
      .unwrap_or_default(),
  })
}

fn resolve_shebang(sys: &impl FsOpen, path: &Path) -> Option<Shebang> {
  let file = sys
    .fs_open(path, sys_traits::OpenOptions::new().read())
    .ok()?;
  let mut reader = std::io::BufReader::new(file);
  let mut line = String::new();
  reader.read_line(&mut line).ok()?;
  parse_shebang(&line)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShimData {
  pub shebang: Option<Shebang>,
  pub target: String,
}

impl ShimData {
  pub fn new(target: impl Into<String>, shebang: Option<Shebang>) -> Self {
    Self {
      shebang,
      target: target.into().replace('\\', "/"),
    }
  }

  fn target_win(&self) -> String {
    self.target.replace('/', "\\")
  }

  pub fn generate_cmd(&self) -> String {
    let mut s = String::with_capacity(512);
    s.push_str(concat!(
      "@ECHO off\r\n",
      "GOTO start\r\n",
      ":find_dp0\r\n",
      "SET dp0=%~dp0\r\n",
      "EXIT /b\r\n",
      ":start\r\n",
      "SETLOCAL\r\n",
      "CALL :find_dp0\r\n",
    ));

    let target_win = self.target_win();
    match &self.shebang {
      None => {
        writeln!(s, "\"%dp0%\\{}\" %*\r", target_win);
      }
      Some(Shebang {
        program,
        args,
        vars,
      }) => {
        let prog = program.replace('\\', "/");
        for var in vars.split_whitespace().filter(|v| v.contains('=')) {
          writeln!(s, "SET {}\r", var);
        }
        let long_prog = format!("%dp0%\\{}.exe", prog);
        writeln!(s, "\r");
        writeln!(s, "IF EXIST \"{}\" (\r", long_prog);
        writeln!(s, "  SET \"_prog={}\"\r", long_prog);
        writeln!(s, ") ELSE (\r");
        writeln!(s, "  SET \"_prog={}\"\r", prog);
        writeln!(s, "  SET PATHEXT=%PATHEXT:;.JS;=;%\r");
        writeln!(s, ")\r");
        writeln!(s, "\r");
        writeln!(
          s,
          "endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\" {} \"%dp0%\\{}\" %*\r",
          args, target_win
        );
      }
    }
    s
  }

  pub fn generate_sh(&self) -> String {
    let mut s = String::with_capacity(512);
    s.push_str(concat!(
      "#!/bin/sh\n",
      "basedir=$(dirname \"$(echo \"$0\" | sed -e 's,\\\\,/,g')\")\n",
      "\n",
      "case `uname` in\n",
      "    *CYGWIN*|*MINGW*|*MSYS*)\n",
      "        if command -v cygpath > /dev/null 2>&1; then\n",
      "            basedir=`cygpath -w \"$basedir\"`\n",
      "        fi\n",
      "    ;;\n",
      "esac\n",
      "\n",
    ));

    let target = format!("\"$basedir/{}\"", self.target);
    match &self.shebang {
      None => {
        writeln!(s, "exec {} \"$@\"", target);
      }
      Some(Shebang {
        program,
        args,
        vars,
      }) => {
        let prog = program.replace('\\', "/");
        let long_prog = format!("\"$basedir/{}\"", prog);
        writeln!(s, "if [ -x {} ]; then", long_prog);
        writeln!(s, "  exec {}{} {} {} \"$@\"", vars, long_prog, args, target);
        writeln!(s, "else");
        writeln!(s, "  exec {}{} {} {} \"$@\"", vars, prog, args, target);
        writeln!(s, "fi");
      }
    }
    s
  }

  pub fn generate_pwsh(&self) -> String {
    let mut s = String::with_capacity(1024);
    s.push_str(concat!(
      "#!/usr/bin/env pwsh\n",
      "$basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent\n",
      "\n",
      "$exe=\"\"\n",
      "if ($PSVersionTable.PSVersion -lt \"6.0\" -or $IsWindows) {\n",
      "  $exe=\".exe\"\n",
      "}\n",
    ));

    let target = format!("\"$basedir/{}\"", self.target);
    match &self.shebang {
      None => {
        Self::write_pwsh_exec(&mut s, &target, "", "");
        writeln!(s, "exit $LASTEXITCODE");
      }
      Some(Shebang { program, args, .. }) => {
        let prog = program.replace('\\', "/");
        let long_prog = format!("\"$basedir/{}$exe\"", prog);
        let short_prog = format!("\"{}$exe\"", prog);
        writeln!(s, "$ret=0");
        writeln!(s, "if (Test-Path {}) {{", long_prog);
        Self::write_pwsh_exec(&mut s, &long_prog, args, &target);
        writeln!(s, "  $ret=$LASTEXITCODE");
        writeln!(s, "}} else {{");
        Self::write_pwsh_exec(&mut s, &short_prog, args, &target);
        writeln!(s, "  $ret=$LASTEXITCODE");
        writeln!(s, "}}");
        writeln!(s, "exit $ret");
      }
    }
    s
  }

  fn write_pwsh_exec(s: &mut String, prog: &str, args: &str, target: &str) {
    writeln!(s, "  if ($MyInvocation.ExpectingInput) {{");
    writeln!(s, "    $input | & {} {} {} $args", prog, args, target);
    writeln!(s, "  }} else {{");
    writeln!(s, "    & {} {} {} $args", prog, args, target);
    writeln!(s, "  }}");
  }
}

fn set_up_bin_shim<'a>(
  sys: &(impl FsOpen + FsWrite),
  #[allow(unused)] package: &'a NpmResolutionPackage,
  #[allow(unused)] extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &'a str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &'a Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  let shim_path = bin_node_modules_dir_path.join(bin_name);
  let target_file = package_path.join(bin_script);

  let rel_target =
    relative_path(bin_node_modules_dir_path, &target_file).unwrap();
  let shebang = resolve_shebang(sys, &target_file);
  let shim = ShimData::new(rel_target.to_string_lossy(), shebang);

  let write_shim = |path: PathBuf, contents: &str| {
    sys
      .fs_write(&path, contents)
      .map_err(|err| BinEntriesError::SetUpBin {
        name: bin_name.to_string(),
        path,
        source: Box::new(err.into()),
      })
  };

  write_shim(shim_path.with_extension("cmd"), &shim.generate_cmd())?;
  write_shim(shim_path.clone(), &shim.generate_sh())?;
  write_shim(shim_path.with_extension("ps1"), &shim.generate_pwsh())?;

  Ok(EntrySetupOutcome::Success)
}

/// Make the file at `path` executable if it exists.
/// Returns `true` if the file exists, `false` otherwise.
fn make_executable_if_exists(
  sys: &impl FsOpen,
  path: &Path,
) -> Result<bool, BinEntriesError> {
  let mut open_options = sys_traits::OpenOptions::new();
  open_options.read = true;
  open_options.write = true;
  open_options.truncate = false; // ensure false
  let mut file = match sys.fs_open(path, &open_options) {
    Ok(file) => file,
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
      return Ok(false);
    }
    Err(err) => return Err(err.into()),
  };
  let metadata = file.fs_file_metadata()?;
  let mode = metadata.mode()?;
  if mode & 0o111 == 0 {
    // if the original file is not executable, make it executable
    file
      .fs_file_set_permissions(mode | 0o111)
      .map_err(|source| BinEntriesError::Permissions {
        path: path.to_path_buf(),
        source,
      })?;
  }

  Ok(true)
}

pub enum EntrySetupOutcome<'a> {
  #[cfg_attr(windows, allow(dead_code))]
  MissingEntrypoint {
    bin_name: &'a str,
    package_path: &'a Path,
    entrypoint: PathBuf,
    package: &'a NpmResolutionPackage,
    extra: &'a NpmPackageExtraInfo,
  },
  Success,
}

impl EntrySetupOutcome<'_> {
  pub fn warn_if_failed(&self) {
    match self {
      EntrySetupOutcome::MissingEntrypoint {
        bin_name,
        package_path,
        entrypoint,
        ..
      } => warn_missing_entrypoint(bin_name, package_path, entrypoint),
      EntrySetupOutcome::Success => {}
    }
  }
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
  pathdiff::diff_paths(to, from)
}

fn symlink_bin_entry<'a>(
  sys: &(impl FsOpen + FsSymlinkFile + FsRemoveFile + FsReadLink),
  package: &'a NpmResolutionPackage,
  extra: &'a NpmPackageExtraInfo,
  bin_name: &'a str,
  bin_script: &str,
  package_path: &'a Path,
  bin_node_modules_dir_path: &Path,
) -> Result<EntrySetupOutcome<'a>, BinEntriesError> {
  let link = bin_node_modules_dir_path.join(bin_name);
  let original = package_path.join(bin_script);

  let original_relative = relative_path(bin_node_modules_dir_path, &original)
    .map(Cow::Owned)
    .unwrap_or_else(|| Cow::Borrowed(&original));

  if let Ok(original_link) = sys.fs_read_link(&link)
    && *original_link == *original_relative
  {
    return Ok(EntrySetupOutcome::Success);
  }

  let found = make_executable_if_exists(sys, &original).map_err(|source| {
    BinEntriesError::SetUpBin {
      name: bin_name.to_string(),
      path: original.to_path_buf(),
      source: Box::new(source),
    }
  })?;
  if !found {
    return Ok(EntrySetupOutcome::MissingEntrypoint {
      bin_name,
      package_path,
      entrypoint: original,
      package,
      extra,
    });
  }

  if let Err(err) = sys.fs_symlink_file(&*original_relative, &link) {
    if err.kind() == std::io::ErrorKind::AlreadyExists {
      // remove and retry
      sys.fs_remove_file(&link).map_err(|source| {
        BinEntriesError::RemoveBinSymlink {
          path: link.clone(),
          source,
        }
      })?;
      sys
        .fs_symlink_file(&*original_relative, &link)
        .map_err(|source| BinEntriesError::SetUpBin {
          name: bin_name.to_string(),
          path: original_relative.to_path_buf(),
          source: Box::new(source.into()),
        })?;
      return Ok(EntrySetupOutcome::Success);
    }
    return Err(BinEntriesError::SetUpBin {
      name: bin_name.to_string(),
      path: original_relative.to_path_buf(),
      source: Box::new(err.into()),
    });
  }

  Ok(EntrySetupOutcome::Success)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Trims the minimum indent from each line of a multiline string,
  /// removing leading and trailing blank lines.
  fn trim_indent(text: &str) -> String {
    trim_indent_with(text, "\n")
  }

  fn trim_indent_crlf(text: &str) -> String {
    trim_indent_with(text, "\r\n")
  }

  fn trim_indent_with(text: &str, line_ending: &str) -> String {
    let text = text.strip_prefix('\n').unwrap_or(text);
    let lines: Vec<&str> = text.lines().collect();
    let min_indent = lines
      .iter()
      .filter(|line| !line.trim().is_empty())
      .map(|line| line.len() - line.trim_start().len())
      .min()
      .unwrap_or(0);

    lines
      .iter()
      .map(|line| {
        if line.len() <= min_indent {
          line.trim_start()
        } else {
          &line[min_indent..]
        }
      })
      .collect::<Vec<_>>()
      .join(line_ending)
  }

  #[test]
  fn test_parse_shebang_node() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env node\n"),
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_with_args() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env node --experimental"),
      Some(Shebang {
        program: "node".into(),
        args: "--experimental".into(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_with_env_vars() {
    assert_eq!(
      parse_shebang("#!/usr/bin/env -S NODE_ENV=production node"),
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: "NODE_ENV=production ".into(), // trailing space is intentional
      })
    );
  }

  #[test]
  fn test_parse_shebang_direct_path() {
    assert_eq!(
      parse_shebang("#!/bin/bash"),
      Some(Shebang {
        program: "/bin/bash".into(),
        args: String::new(),
        vars: String::new(),
      })
    );
  }

  #[test]
  fn test_parse_shebang_invalid() {
    assert_eq!(parse_shebang("not a shebang"), None);
    assert_eq!(parse_shebang(""), None);
  }

  #[test]
  fn test_sh_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_sh(),
      trim_indent(
        r#"
        #!/bin/sh
        basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

        case `uname` in
            *CYGWIN*|*MINGW*|*MSYS*)
                if command -v cygpath > /dev/null 2>&1; then
                    basedir=`cygpath -w "$basedir"`
                fi
            ;;
        esac

        exec "$basedir/../pkg/bin/cli.js" "$@"
        "#
      )
    );
  }

  #[test]
  fn test_sh_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_sh(),
      trim_indent(
        r#"
        #!/bin/sh
        basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")

        case `uname` in
            *CYGWIN*|*MINGW*|*MSYS*)
                if command -v cygpath > /dev/null 2>&1; then
                    basedir=`cygpath -w "$basedir"`
                fi
            ;;
        esac

        if [ -x "$basedir/node" ]; then
          exec "$basedir/node"  "$basedir/../pkg/bin/cli.js" "$@"
        else
          exec node  "$basedir/../pkg/bin/cli.js" "$@"
        fi
        "#
      )
    );
  }

  #[test]
  fn test_cmd_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_cmd(),
      trim_indent_crlf(
        r#"
        @ECHO off
        GOTO start
        :find_dp0
        SET dp0=%~dp0
        EXIT /b
        :start
        SETLOCAL
        CALL :find_dp0
        "%dp0%\..\pkg\bin\cli.js" %*
        "#
      )
    );
  }

  #[test]
  fn test_cmd_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_cmd(),
      trim_indent_crlf(
        r#"
        @ECHO off
        GOTO start
        :find_dp0
        SET dp0=%~dp0
        EXIT /b
        :start
        SETLOCAL
        CALL :find_dp0

        IF EXIST "%dp0%\node.exe" (
          SET "_prog=%dp0%\node.exe"
        ) ELSE (
          SET "_prog=node"
          SET PATHEXT=%PATHEXT:;.JS;=;%
        )

        endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\..\pkg\bin\cli.js" %*
        "#
      )
    );
  }

  #[test]
  fn test_pwsh_shim_raw() {
    let shim = ShimData::new("../pkg/bin/cli.js", None);
    assert_eq!(
      shim.generate_pwsh(),
      trim_indent(
        r#"
        #!/usr/bin/env pwsh
        $basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent

        $exe=""
        if ($PSVersionTable.PSVersion -lt "6.0" -or $IsWindows) {
          $exe=".exe"
        }
          if ($MyInvocation.ExpectingInput) {
            $input | & "$basedir/../pkg/bin/cli.js"   $args
          } else {
            & "$basedir/../pkg/bin/cli.js"   $args
          }
        exit $LASTEXITCODE
        "#
      )
    );
  }

  #[test]
  fn test_pwsh_shim_with_program() {
    let shim = ShimData::new(
      "../pkg/bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: String::new(),
        vars: String::new(),
      }),
    );
    assert_eq!(
      shim.generate_pwsh(),
      trim_indent(
        r#"
        #!/usr/bin/env pwsh
        $basedir=Split-Path $MyInvocation.MyCommand.Definition -Parent

        $exe=""
        if ($PSVersionTable.PSVersion -lt "6.0" -or $IsWindows) {
          $exe=".exe"
        }
        $ret=0
        if (Test-Path "$basedir/node$exe") {
          if ($MyInvocation.ExpectingInput) {
            $input | & "$basedir/node$exe"  "$basedir/../pkg/bin/cli.js" $args
          } else {
            & "$basedir/node$exe"  "$basedir/../pkg/bin/cli.js" $args
          }
          $ret=$LASTEXITCODE
        } else {
          if ($MyInvocation.ExpectingInput) {
            $input | & "node$exe"  "$basedir/../pkg/bin/cli.js" $args
          } else {
            & "node$exe"  "$basedir/../pkg/bin/cli.js" $args
          }
          $ret=$LASTEXITCODE
        }
        exit $ret
        "#
      )
    );
  }

  #[test]
  fn test_shim_with_args_and_vars() {
    let shim = ShimData::new(
      "bin/cli.js",
      Some(Shebang {
        program: "node".into(),
        args: "--experimental-modules".into(),
        vars: "NODE_ENV=prod ".into(), // trailing space is intentional
      }),
    );

    let sh = shim.generate_sh();
    assert!(
      sh.contains("NODE_ENV=prod \"$basedir/node\" --experimental-modules")
    );
    assert!(sh.contains("NODE_ENV=prod node --experimental-modules"));

    let cmd = shim.generate_cmd();
    assert!(cmd.contains("SET NODE_ENV=prod\r\n"));
    assert!(cmd.contains("\"%_prog%\" --experimental-modules"));

    let pwsh = shim.generate_pwsh();
    assert!(pwsh.contains("\"$basedir/node$exe\" --experimental-modules"));
  }
}
