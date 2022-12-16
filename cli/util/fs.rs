// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use deno_core::anyhow::Context;
use deno_core::error::AnyError;
pub use deno_core::normalize_path;
use deno_core::ModuleSpecifier;
use deno_runtime::deno_crypto::rand;
use deno_runtime::deno_node::PathClean;
use std::env::current_dir;
use std::fs::OpenOptions;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use walkdir::WalkDir;

use super::path::specifier_to_file_path;

pub fn atomic_write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  let rand: String = (0..4)
    .map(|_| format!("{:02x}", rand::random::<u8>()))
    .collect();
  let extension = format!("{}.tmp", rand);
  let tmp_file = filename.with_extension(extension);
  write_file(&tmp_file, data, mode)?;
  std::fs::rename(tmp_file, filename)?;
  Ok(())
}

pub fn write_file<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  mode: u32,
) -> std::io::Result<()> {
  write_file_2(filename, data, true, mode, true, false)
}

pub fn write_file_2<T: AsRef<[u8]>>(
  filename: &Path,
  data: T,
  update_mode: bool,
  mode: u32,
  is_create: bool,
  is_append: bool,
) -> std::io::Result<()> {
  let mut file = OpenOptions::new()
    .read(false)
    .write(true)
    .append(is_append)
    .truncate(!is_append)
    .create(is_create)
    .open(filename)?;

  if update_mode {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let mode = mode & 0o777;
      let permissions = PermissionsExt::from_mode(mode);
      file.set_permissions(permissions)?;
    }
    #[cfg(not(unix))]
    let _ = mode;
  }

  file.write_all(data.as_ref())
}

/// Similar to `std::fs::canonicalize()` but strips UNC prefixes on Windows.
pub fn canonicalize_path(path: &Path) -> Result<PathBuf, Error> {
  let path = path.canonicalize()?;
  #[cfg(windows)]
  return Ok(strip_unc_prefix(path));
  #[cfg(not(windows))]
  return Ok(path);
}

/// Canonicalizes a path which might be non-existent by going up the
/// ancestors until it finds a directory that exists, canonicalizes
/// that path, then adds back the remaining path components.
///
/// Note: When using this, you should be aware that a symlink may
/// subsequently be created along this path by some other code.
pub fn canonicalize_path_maybe_not_exists(
  path: &Path,
) -> Result<PathBuf, Error> {
  let path = path.to_path_buf().clean();
  let mut path = path.as_path();
  let mut names_stack = Vec::new();
  loop {
    match canonicalize_path(path) {
      Ok(mut canonicalized_path) => {
        for name in names_stack.into_iter().rev() {
          canonicalized_path = canonicalized_path.join(name);
        }
        return Ok(canonicalized_path);
      }
      Err(err) if err.kind() == ErrorKind::NotFound => {
        names_stack.push(path.file_name().unwrap());
        path = path.parent().unwrap();
      }
      Err(err) => return Err(err),
    }
  }
}

#[cfg(windows)]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  use std::path::Component;
  use std::path::Prefix;

  let mut components = path.components();
  match components.next() {
    Some(Component::Prefix(prefix)) => {
      match prefix.kind() {
        // \\?\device
        Prefix::Verbatim(device) => {
          let mut path = PathBuf::new();
          path.push(format!(r"\\{}\", device.to_string_lossy()));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        // \\?\c:\path
        Prefix::VerbatimDisk(_) => {
          let mut path = PathBuf::new();
          path.push(prefix.as_os_str().to_string_lossy().replace(r"\\?\", ""));
          path.extend(components);
          path
        }
        // \\?\UNC\hostname\share_name\path
        Prefix::VerbatimUNC(hostname, share_name) => {
          let mut path = PathBuf::new();
          path.push(format!(
            r"\\{}\{}\",
            hostname.to_string_lossy(),
            share_name.to_string_lossy()
          ));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        _ => path,
      }
    }
    _ => path,
  }
}

pub fn resolve_from_cwd(path: &Path) -> Result<PathBuf, AnyError> {
  let resolved_path = if path.is_absolute() {
    path.to_owned()
  } else {
    let cwd =
      current_dir().context("Failed to get current working directory")?;
    cwd.join(path)
  };

  Ok(normalize_path(resolved_path))
}

/// Collects file paths that satisfy the given predicate, by recursively walking `files`.
/// If the walker visits a path that is listed in `ignore`, it skips descending into the directory.
pub struct FileCollector<TFilter: Fn(&Path) -> bool> {
  canonicalized_ignore: Vec<PathBuf>,
  file_filter: TFilter,
  ignore_git_folder: bool,
  ignore_node_modules: bool,
}

impl<TFilter: Fn(&Path) -> bool> FileCollector<TFilter> {
  pub fn new(file_filter: TFilter) -> Self {
    Self {
      canonicalized_ignore: Default::default(),
      file_filter,
      ignore_git_folder: false,
      ignore_node_modules: false,
    }
  }
  pub fn add_ignore_paths(mut self, paths: &[PathBuf]) -> Self {
    // retain only the paths which exist and ignore the rest
    self
      .canonicalized_ignore
      .extend(paths.iter().filter_map(|i| canonicalize_path(i).ok()));
    self
  }

  pub fn ignore_node_modules(mut self) -> Self {
    self.ignore_node_modules = true;
    self
  }

  pub fn ignore_git_folder(mut self) -> Self {
    self.ignore_git_folder = true;
    self
  }

  pub fn collect_files(
    &self,
    files: &[PathBuf],
  ) -> Result<Vec<PathBuf>, AnyError> {
    let mut target_files = Vec::new();
    for file in files {
      if let Ok(file) = canonicalize_path(file) {
        // use an iterator like this in order to minimize the number of file system operations
        let mut iterator = WalkDir::new(&file).into_iter();
        loop {
          let e = match iterator.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(entry)) => entry,
          };
          let file_type = e.file_type();
          let is_dir = file_type.is_dir();
          if let Ok(c) = canonicalize_path(e.path()) {
            if self.canonicalized_ignore.iter().any(|i| c.starts_with(i)) {
              if is_dir {
                iterator.skip_current_dir();
              }
            } else if is_dir {
              let should_ignore_dir = c
                .file_name()
                .map(|dir_name| {
                  let dir_name = dir_name.to_string_lossy().to_lowercase();
                  let is_ignored_file = self.ignore_node_modules
                    && dir_name == "node_modules"
                    || self.ignore_git_folder && dir_name == ".git";
                  // allow the user to opt out of ignoring by explicitly specifying the dir
                  file != c && is_ignored_file
                })
                .unwrap_or(false);
              if should_ignore_dir {
                iterator.skip_current_dir();
              }
            } else if (self.file_filter)(e.path()) {
              target_files.push(c);
            }
          } else if is_dir {
            // failed canonicalizing, so skip it
            iterator.skip_current_dir();
          }
        }
      }
    }
    Ok(target_files)
  }
}

/// Collects module specifiers that satisfy the given predicate as a file path, by recursively walking `include`.
/// Specifiers that start with http and https are left intact.
/// Note: This ignores all .git and node_modules folders.
pub fn collect_specifiers(
  include: Vec<String>,
  ignore: &[PathBuf],
  predicate: impl Fn(&Path) -> bool,
) -> Result<Vec<ModuleSpecifier>, AnyError> {
  let mut prepared = vec![];
  let file_collector = FileCollector::new(predicate)
    .add_ignore_paths(ignore)
    .ignore_git_folder()
    .ignore_node_modules();

  let root_path = current_dir()?;
  for path in include {
    let lowercase_path = path.to_lowercase();
    if lowercase_path.starts_with("http://")
      || lowercase_path.starts_with("https://")
    {
      let url = ModuleSpecifier::parse(&path)?;
      prepared.push(url);
      continue;
    }

    let p = if lowercase_path.starts_with("file://") {
      specifier_to_file_path(&ModuleSpecifier::parse(&path)?)?
    } else {
      root_path.join(path)
    };
    let p = normalize_path(p);
    if p.is_dir() {
      let test_files = file_collector.collect_files(&[p])?;
      let mut test_files_as_urls = test_files
        .iter()
        .map(|f| ModuleSpecifier::from_file_path(f).unwrap())
        .collect::<Vec<ModuleSpecifier>>();

      test_files_as_urls.sort();
      prepared.extend(test_files_as_urls);
    } else {
      let url = ModuleSpecifier::from_file_path(p).unwrap();
      prepared.push(url);
    }
  }

  Ok(prepared)
}

/// Asynchronously removes a directory and all its descendants, but does not error
/// when the directory does not exist.
pub async fn remove_dir_all_if_exists(path: &Path) -> std::io::Result<()> {
  let result = tokio::fs::remove_dir_all(path).await;
  match result {
    Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
    _ => result,
  }
}

/// Copies a directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(to)
    .with_context(|| format!("Creating {}", to.display()))?;
  let read_dir = std::fs::read_dir(from)
    .with_context(|| format!("Reading {}", from.display()))?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      copy_dir_recursive(&new_from, &new_to).with_context(|| {
        format!("Dir {} to {}", new_from.display(), new_to.display())
      })?;
    } else if file_type.is_file() {
      std::fs::copy(&new_from, &new_to).with_context(|| {
        format!("Copying {} to {}", new_from.display(), new_to.display())
      })?;
    }
  }

  Ok(())
}

/// Hardlinks the files in one directory to another directory.
///
/// Note: Does not handle symlinks.
pub fn hard_link_dir_recursive(from: &Path, to: &Path) -> Result<(), AnyError> {
  std::fs::create_dir_all(to)
    .with_context(|| format!("Creating {}", to.display()))?;
  let read_dir = std::fs::read_dir(from)
    .with_context(|| format!("Reading {}", from.display()))?;

  for entry in read_dir {
    let entry = entry?;
    let file_type = entry.file_type()?;
    let new_from = from.join(entry.file_name());
    let new_to = to.join(entry.file_name());

    if file_type.is_dir() {
      hard_link_dir_recursive(&new_from, &new_to).with_context(|| {
        format!("Dir {} to {}", new_from.display(), new_to.display())
      })?;
    } else if file_type.is_file() {
      // note: chance for race conditions here between attempting to create,
      // then removing, then attempting to create. There doesn't seem to be
      // a way to hard link with overwriting in Rust, but maybe there is some
      // way with platform specific code. The workaround here is to handle
      // scenarios where something else might create or remove files.
      if let Err(err) = std::fs::hard_link(&new_from, &new_to) {
        if err.kind() == ErrorKind::AlreadyExists {
          if let Err(err) = std::fs::remove_file(&new_to) {
            if err.kind() == ErrorKind::NotFound {
              // Assume another process/thread created this hard link to the file we are wanting
              // to remove then sleep a little bit to let the other process/thread move ahead
              // faster to reduce contention.
              std::thread::sleep(Duration::from_millis(10));
            } else {
              return Err(err).with_context(|| {
                format!(
                  "Removing file to hard link {} to {}",
                  new_from.display(),
                  new_to.display()
                )
              });
            }
          }

          // Always attempt to recreate the hardlink. In contention scenarios, the other process
          // might have been killed or exited after removing the file, but before creating the hardlink
          if let Err(err) = std::fs::hard_link(&new_from, &new_to) {
            // Assume another process/thread created this hard link to the file we are wanting
            // to now create then sleep a little bit to let the other process/thread move ahead
            // faster to reduce contention.
            if err.kind() == ErrorKind::AlreadyExists {
              std::thread::sleep(Duration::from_millis(10));
            } else {
              return Err(err).with_context(|| {
                format!(
                  "Hard linking {} to {}",
                  new_from.display(),
                  new_to.display()
                )
              });
            }
          }
        } else {
          return Err(err).with_context(|| {
            format!(
              "Hard linking {} to {}",
              new_from.display(),
              new_to.display()
            )
          });
        }
      }
    }
  }

  Ok(())
}

pub fn symlink_dir(oldpath: &Path, newpath: &Path) -> Result<(), AnyError> {
  let err_mapper = |err: Error| {
    Error::new(
      err.kind(),
      format!(
        "{}, symlink '{}' -> '{}'",
        err,
        oldpath.display(),
        newpath.display()
      ),
    )
  };
  #[cfg(unix)]
  {
    use std::os::unix::fs::symlink;
    symlink(oldpath, newpath).map_err(err_mapper)?;
  }
  #[cfg(not(unix))]
  {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(oldpath, newpath).map_err(err_mapper)?;
  }
  Ok(())
}

/// Gets the total size (in bytes) of a directory.
pub fn dir_size(path: &Path) -> std::io::Result<u64> {
  let entries = std::fs::read_dir(path)?;
  let mut total = 0;
  for entry in entries {
    let entry = entry?;
    total += match entry.metadata()? {
      data if data.is_dir() => dir_size(&entry.path())?,
      data => data.len(),
    };
  }
  Ok(total)
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;
  use test_util::TempDir;

  #[test]
  fn resolve_from_cwd_child() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a")).unwrap(), cwd.join("a"));
  }

  #[test]
  fn resolve_from_cwd_dot() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new(".")).unwrap(), cwd);
  }

  #[test]
  fn resolve_from_cwd_parent() {
    let cwd = current_dir().unwrap();
    assert_eq!(resolve_from_cwd(Path::new("a/..")).unwrap(), cwd);
  }

  #[test]
  fn test_normalize_path() {
    assert_eq!(normalize_path(Path::new("a/../b")), PathBuf::from("b"));
    assert_eq!(normalize_path(Path::new("a/./b/")), PathBuf::from("a/b/"));
    assert_eq!(
      normalize_path(Path::new("a/./b/../c")),
      PathBuf::from("a/c")
    );

    if cfg!(windows) {
      assert_eq!(
        normalize_path(Path::new("C:\\a\\.\\b\\..\\c")),
        PathBuf::from("C:\\a\\c")
      );
    }
  }

  // TODO: Get a good expected value here for Windows.
  #[cfg(not(windows))]
  #[test]
  fn resolve_from_cwd_absolute() {
    let expected = Path::new("/a");
    assert_eq!(resolve_from_cwd(expected).unwrap(), expected);
  }

  #[test]
  fn test_collect_files() {
    fn create_files(dir_path: &Path, files: &[&str]) {
      std::fs::create_dir(dir_path).expect("Failed to create directory");
      for f in files {
        let path = dir_path.join(f);
        std::fs::write(path, "").expect("Failed to create file");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // |   ├── node_modules
    // |   |   └── node_modules.js
    // |   ├── git
    // |   |   └── git.js
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    t.create_dir_all("dir.ts/child/node_modules");
    t.write("dir.ts/child/node_modules/node_modules.js", "");
    t.create_dir_all("dir.ts/child/.git");
    t.write("dir.ts/child/.git/git.js", "");

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let file_collector = FileCollector::new(|path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map_or(false, |f| !f.starts_with('.'))
    })
    .add_ignore_paths(&[ignore_dir_path]);

    let result = file_collector
      .collect_files(&[root_dir_path.clone()])
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "git.js",
      "node_modules.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test ignoring the .git and node_modules folder
    let file_collector =
      file_collector.ignore_git_folder().ignore_node_modules();
    let result = file_collector
      .collect_files(&[root_dir_path.clone()])
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);

    // test opting out of ignoring by specifying the dir
    let result = file_collector
      .collect_files(&[
        root_dir_path.clone(),
        root_dir_path.join("child/node_modules/"),
      ])
      .unwrap();
    let expected = [
      "README.md",
      "a.ts",
      "b.js",
      "c.tsx",
      "d.jsx",
      "e.mjs",
      "f.mjsx",
      "node_modules.js",
    ];
    let mut file_names = result
      .into_iter()
      .map(|r| r.file_name().unwrap().to_string_lossy().to_string())
      .collect::<Vec<_>>();
    file_names.sort();
    assert_eq!(file_names, expected);
  }

  #[test]
  fn test_collect_specifiers() {
    fn create_files(dir_path: &Path, files: &[&str]) {
      std::fs::create_dir(dir_path).expect("Failed to create directory");
      for f in files {
        let path = dir_path.join(f);
        std::fs::write(path, "").expect("Failed to create file");
      }
    }

    // dir.ts
    // ├── a.ts
    // ├── b.js
    // ├── child
    // │   ├── e.mjs
    // │   ├── f.mjsx
    // │   ├── .foo.TS
    // │   └── README.md
    // ├── c.tsx
    // ├── d.jsx
    // └── ignore
    //     ├── g.d.ts
    //     └── .gitignore

    let t = TempDir::new();

    let root_dir_path = t.path().join("dir.ts");
    let root_dir_files = ["a.ts", "b.js", "c.tsx", "d.jsx"];
    create_files(&root_dir_path, &root_dir_files);

    let child_dir_path = root_dir_path.join("child");
    let child_dir_files = ["e.mjs", "f.mjsx", ".foo.TS", "README.md"];
    create_files(&child_dir_path, &child_dir_files);

    let ignore_dir_path = root_dir_path.join("ignore");
    let ignore_dir_files = ["g.d.ts", ".gitignore"];
    create_files(&ignore_dir_path, &ignore_dir_files);

    let predicate = |path: &Path| {
      // exclude dotfiles
      path
        .file_name()
        .and_then(|f| f.to_str())
        .map_or(false, |f| !f.starts_with('.'))
    };

    let result = collect_specifiers(
      vec![
        "http://localhost:8080".to_string(),
        root_dir_path.to_str().unwrap().to_string(),
        "https://localhost:8080".to_string(),
      ],
      &[ignore_dir_path],
      predicate,
    )
    .unwrap();

    let root_dir_url = ModuleSpecifier::from_file_path(
      canonicalize_path(&root_dir_path).unwrap(),
    )
    .unwrap()
    .to_string();
    let expected: Vec<ModuleSpecifier> = [
      "http://localhost:8080",
      &format!("{}/a.ts", root_dir_url),
      &format!("{}/b.js", root_dir_url),
      &format!("{}/c.tsx", root_dir_url),
      &format!("{}/child/README.md", root_dir_url),
      &format!("{}/child/e.mjs", root_dir_url),
      &format!("{}/child/f.mjsx", root_dir_url),
      &format!("{}/d.jsx", root_dir_url),
      "https://localhost:8080",
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);

    let scheme = if cfg!(target_os = "windows") {
      "file:///"
    } else {
      "file://"
    };
    let result = collect_specifiers(
      vec![format!(
        "{}{}",
        scheme,
        root_dir_path
          .join("child")
          .to_str()
          .unwrap()
          .replace('\\', "/")
      )],
      &[],
      predicate,
    )
    .unwrap();

    let expected: Vec<ModuleSpecifier> = [
      &format!("{}/child/README.md", root_dir_url),
      &format!("{}/child/e.mjs", root_dir_url),
      &format!("{}/child/f.mjsx", root_dir_url),
    ]
    .iter()
    .map(|f| ModuleSpecifier::parse(f).unwrap())
    .collect::<Vec<_>>();

    assert_eq!(result, expected);
  }

  #[cfg(windows)]
  #[test]
  fn test_strip_unc_prefix() {
    run_test(r"C:\", r"C:\");
    run_test(r"C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\?\C:\", r"C:\");
    run_test(r"\\?\C:\test\file.txt", r"C:\test\file.txt");

    run_test(r"\\.\C:\", r"\\.\C:\");
    run_test(r"\\.\C:\Test\file.txt", r"\\.\C:\Test\file.txt");

    run_test(r"\\?\UNC\localhost\", r"\\localhost");
    run_test(r"\\?\UNC\localhost\c$\", r"\\localhost\c$");
    run_test(
      r"\\?\UNC\localhost\c$\Windows\file.txt",
      r"\\localhost\c$\Windows\file.txt",
    );
    run_test(r"\\?\UNC\wsl$\deno.json", r"\\wsl$\deno.json");

    run_test(r"\\?\server1", r"\\server1");
    run_test(r"\\?\server1\e$\", r"\\server1\e$\");
    run_test(
      r"\\?\server1\e$\test\file.txt",
      r"\\server1\e$\test\file.txt",
    );

    fn run_test(input: &str, expected: &str) {
      assert_eq!(
        strip_unc_prefix(PathBuf::from(input)),
        PathBuf::from(expected)
      );
    }
  }
}
