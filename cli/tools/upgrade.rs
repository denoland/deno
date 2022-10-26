// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

//! This module provides feature to upgrade deno executable

use crate::args::UpgradeFlags;
use crate::colors;
use crate::version;

use deno_core::anyhow::bail;
use deno_core::error::AnyError;
use deno_core::futures::future::BoxFuture;
use deno_core::futures::FutureExt;
use deno_core::futures::StreamExt;
use deno_runtime::deno_fetch::reqwest;
use deno_runtime::deno_fetch::reqwest::Client;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::env;
use std::fs;
use std::io::Write;
use std::ops::Sub;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

static ARCHIVE_NAME: Lazy<String> =
  Lazy::new(|| format!("deno-{}.zip", env!("TARGET")));

const RELEASE_URL: &str = "https://github.com/denoland/deno/releases";

// How often query server for new version. In hours.
const UPGRADE_CHECK_INTERVAL: i64 = 24;
const UPGRADE_CHECK_FILE_NAME: &str = "latest.txt";

const UPGRADE_CHECK_FETCH_DELAY: Duration = Duration::from_millis(500);

/// Environment necessary for doing the update checker.
/// An alternate trait implementation can be provided for testing purposes.
trait UpdateCheckerEnvironment: Clone + Send + Sync {
  fn latest_version(&self) -> BoxFuture<'static, Result<String, AnyError>>;
  fn current_version(&self) -> Cow<str>;
  fn read_check_file(&self) -> String;
  fn write_check_file(&self, text: &str);
  fn current_time(&self) -> chrono::DateTime<chrono::Utc>;
}

#[derive(Clone)]
struct RealUpdateCheckerEnvironment {
  cache_dir: PathBuf,
  current_time: chrono::DateTime<chrono::Utc>,
}

impl RealUpdateCheckerEnvironment {
  pub fn new(cache_dir: PathBuf) -> Self {
    Self {
      cache_dir,
      // cache the current time
      current_time: chrono::Utc::now(),
    }
  }
}

impl UpdateCheckerEnvironment for RealUpdateCheckerEnvironment {
  fn latest_version(&self) -> BoxFuture<'static, Result<String, AnyError>> {
    async {
      let client = build_http_client(None)?;
      if version::is_canary() {
        get_latest_canary_version(&client).await
      } else {
        get_latest_release_version(&client).await
      }
    }
    .boxed()
  }

  fn current_version(&self) -> Cow<str> {
    Cow::Borrowed(version::release_version_or_canary_commit_hash())
  }

  fn read_check_file(&self) -> String {
    std::fs::read_to_string(self.cache_dir.join(UPGRADE_CHECK_FILE_NAME))
      .unwrap_or_default()
  }

  fn write_check_file(&self, text: &str) {
    let _ = std::fs::write(self.cache_dir.join(UPGRADE_CHECK_FILE_NAME), text);
  }

  fn current_time(&self) -> chrono::DateTime<chrono::Utc> {
    self.current_time
  }
}

struct UpdateChecker<TEnvironment: UpdateCheckerEnvironment> {
  env: TEnvironment,
  maybe_file: Option<CheckVersionFile>,
}

impl<TEnvironment: UpdateCheckerEnvironment> UpdateChecker<TEnvironment> {
  pub fn new(env: TEnvironment) -> Self {
    let maybe_file = CheckVersionFile::parse(env.read_check_file());
    Self { env, maybe_file }
  }

  pub fn should_check_for_new_version(&self) -> bool {
    match &self.maybe_file {
      Some(file) => {
        let last_check_age = self
          .env
          .current_time()
          .signed_duration_since(file.last_checked);
        last_check_age > chrono::Duration::hours(UPGRADE_CHECK_INTERVAL)
      }
      None => true,
    }
  }

  /// Returns the version if a new one is available and it should be prompted about.
  pub fn should_prompt(&self) -> Option<String> {
    let file = self.maybe_file.as_ref()?;
    if file.latest_version == self.env.current_version() {
      return None;
    }

    let last_prompt_age = self
      .env
      .current_time()
      .signed_duration_since(file.last_prompt);
    if last_prompt_age > chrono::Duration::hours(UPGRADE_CHECK_INTERVAL) {
      Some(file.latest_version.clone())
    } else {
      None
    }
  }

  /// Store that we showed the update message to the user.
  pub fn store_prompted(self) {
    if let Some(file) = self.maybe_file {
      self.env.write_check_file(
        &file.with_last_prompt(self.env.current_time()).serialize(),
      );
    }
  }
}

pub fn check_for_upgrades(cache_dir: PathBuf) {
  if env::var("DENO_NO_UPDATE_CHECK").is_ok() {
    return;
  }

  let env = RealUpdateCheckerEnvironment::new(cache_dir);
  let update_checker = UpdateChecker::new(env);

  if update_checker.should_check_for_new_version() {
    let env = update_checker.env.clone();
    // do this asynchronously on a separate task
    tokio::spawn(async move {
      // Sleep for a small amount of time to not unnecessarily impact startup
      // time.
      tokio::time::sleep(UPGRADE_CHECK_FETCH_DELAY).await;

      fetch_and_store_latest_version(&env).await;
    });
  }

  // Print a message if an update is available, unless:
  //   * stderr is not a tty
  //   * we're already running the 'deno upgrade' command.
  if let Some(upgrade_version) = update_checker.should_prompt() {
    if atty::is(atty::Stream::Stderr) {
      eprint!(
        "{} ",
        colors::green(format!("Deno {upgrade_version} has been released."))
      );
      eprintln!(
        "{}",
        colors::italic_gray("Run `deno upgrade` to install it.")
      );

      update_checker.store_prompted();
    }
  }
}

async fn fetch_and_store_latest_version<
  TEnvironment: UpdateCheckerEnvironment,
>(
  env: &TEnvironment,
) {
  // Fetch latest version or commit hash from server.
  let latest_version = match env.latest_version().await {
    Ok(latest_version) => latest_version,
    Err(_) => return,
  };

  env.write_check_file(
    &CheckVersionFile {
      // put a date in the past here so that prompt can be shown on next run
      last_prompt: env
        .current_time()
        .sub(chrono::Duration::hours(UPGRADE_CHECK_INTERVAL + 1)),
      last_checked: env.current_time(),
      latest_version,
    }
    .serialize(),
  );
}

pub async fn upgrade(upgrade_flags: UpgradeFlags) -> Result<(), AnyError> {
  let old_exe_path = std::env::current_exe()?;
  let metadata = fs::metadata(&old_exe_path)?;
  let permissions = metadata.permissions();

  if permissions.readonly() {
    bail!(
      "You do not have write permission to {}",
      old_exe_path.display()
    );
  }
  #[cfg(unix)]
  if std::os::unix::fs::MetadataExt::uid(&metadata) == 0
    && !nix::unistd::Uid::effective().is_root()
  {
    bail!(concat!(
      "You don't have write permission to {} because it's owned by root.\n",
      "Consider updating deno through your package manager if its installed from it.\n",
      "Otherwise run `deno upgrade` as root.",
    ), old_exe_path.display());
  }

  let client = build_http_client(upgrade_flags.ca_file)?;

  let install_version = match upgrade_flags.version {
    Some(passed_version) => {
      if upgrade_flags.canary
        && !regex::Regex::new("^[0-9a-f]{40}$")?.is_match(&passed_version)
      {
        bail!("Invalid commit hash passed");
      } else if !upgrade_flags.canary
        && semver::Version::parse(&passed_version).is_err()
      {
        bail!("Invalid semver passed");
      }

      let current_is_passed = if upgrade_flags.canary {
        crate::version::GIT_COMMIT_HASH == passed_version
      } else if !crate::version::is_canary() {
        crate::version::deno() == passed_version
      } else {
        false
      };

      if !upgrade_flags.force
        && upgrade_flags.output.is_none()
        && current_is_passed
      {
        println!("Version {} is already installed", crate::version::deno());
        return Ok(());
      } else {
        passed_version
      }
    }
    None => {
      let latest_version = if upgrade_flags.canary {
        println!("Looking up latest canary version");
        get_latest_canary_version(&client).await?
      } else {
        println!("Looking up latest version");
        get_latest_release_version(&client).await?
      };

      let current_is_most_recent = if upgrade_flags.canary {
        let latest_hash = latest_version.clone();
        crate::version::GIT_COMMIT_HASH == latest_hash
      } else if !crate::version::is_canary() {
        let current = semver::Version::parse(&crate::version::deno()).unwrap();
        let latest = semver::Version::parse(&latest_version).unwrap();
        current >= latest
      } else {
        false
      };

      if !upgrade_flags.force
        && upgrade_flags.output.is_none()
        && current_is_most_recent
      {
        println!(
          "Local deno version {} is the most recent release",
          crate::version::deno()
        );
        return Ok(());
      } else {
        println!("Found latest version {}", latest_version);
        latest_version
      }
    }
  };

  let download_url = if upgrade_flags.canary {
    if env!("TARGET") == "aarch64-apple-darwin" {
      bail!("Canary builds are not available for M1");
    }

    format!(
      "https://dl.deno.land/canary/{}/{}",
      install_version, *ARCHIVE_NAME
    )
  } else {
    format!(
      "{}/download/v{}/{}",
      RELEASE_URL, install_version, *ARCHIVE_NAME
    )
  };

  let archive_data = download_package(client, &download_url).await?;

  println!("Deno is upgrading to version {}", &install_version);

  let new_exe_path = unpack(archive_data, cfg!(windows))?;
  fs::set_permissions(&new_exe_path, permissions)?;
  check_exe(&new_exe_path)?;

  if !upgrade_flags.dry_run {
    match upgrade_flags.output {
      Some(path) => {
        fs::rename(&new_exe_path, &path)
          .or_else(|_| fs::copy(&new_exe_path, &path).map(|_| ()))?;
      }
      None => replace_exe(&new_exe_path, &old_exe_path)?,
    }
  }

  println!("Upgraded successfully");

  Ok(())
}

fn build_http_client(
  ca_file: Option<String>,
) -> Result<reqwest::Client, AnyError> {
  let mut client_builder =
    Client::builder().user_agent(version::get_user_agent());

  // If we have been provided a CA Certificate, add it into the HTTP client
  let ca_file = ca_file.or_else(|| env::var("DENO_CERT").ok());
  if let Some(ca_file) = ca_file {
    let buf = std::fs::read(ca_file)?;
    let cert = reqwest::Certificate::from_pem(&buf)?;
    client_builder = client_builder.add_root_certificate(cert);
  }

  let client = client_builder.build()?;

  Ok(client)
}

async fn get_latest_release_version(
  client: &Client,
) -> Result<String, AnyError> {
  let res = client
    .get("https://dl.deno.land/release-latest.txt")
    .send()
    .await?;
  let version = res.text().await?.trim().to_string();
  Ok(version.replace('v', ""))
}

async fn get_latest_canary_version(
  client: &Client,
) -> Result<String, AnyError> {
  let res = client
    .get("https://dl.deno.land/canary-latest.txt")
    .send()
    .await?;
  let version = res.text().await?.trim().to_string();
  Ok(version)
}

async fn download_package(
  client: Client,
  download_url: &str,
) -> Result<Vec<u8>, AnyError> {
  println!("Checking {}", &download_url);

  let res = client.get(download_url).send().await?;

  if res.status().is_success() {
    let total_size = res.content_length().unwrap() as f64;
    let mut current_size = 0.0;
    let mut data = Vec::with_capacity(total_size as usize);
    let mut stream = res.bytes_stream();
    let mut skip_print = 0;
    let is_tty = atty::is(atty::Stream::Stdout);
    const MEBIBYTE: f64 = 1024.0 * 1024.0;
    while let Some(item) = stream.next().await {
      let bytes = item?;
      current_size += bytes.len() as f64;
      data.extend_from_slice(&bytes);
      if skip_print == 0 {
        if is_tty {
          print!("\u{001b}[1G\u{001b}[2K");
        }
        print!(
          "{:>4.1} MiB / {:.1} MiB ({:^5.1}%)",
          current_size / MEBIBYTE,
          total_size / MEBIBYTE,
          (current_size / total_size) * 100.0,
        );
        std::io::stdout().flush()?;
        skip_print = 10;
      } else {
        skip_print -= 1;
      }
    }
    if is_tty {
      print!("\u{001b}[1G\u{001b}[2K");
    }
    println!(
      "{:.1} MiB / {:.1} MiB (100.0%)",
      current_size / MEBIBYTE,
      total_size / MEBIBYTE
    );

    Ok(data)
  } else {
    println!("Download could not be found, aborting");
    std::process::exit(1)
  }
}

pub fn unpack(
  archive_data: Vec<u8>,
  is_windows: bool,
) -> Result<PathBuf, std::io::Error> {
  const EXE_NAME: &str = "deno";
  // We use into_path so that the tempdir is not automatically deleted. This is
  // useful for debugging upgrade, but also so this function can return a path
  // to the newly uncompressed file without fear of the tempdir being deleted.
  let temp_dir = secure_tempfile::TempDir::new()?.into_path();
  let exe_ext = if is_windows { "exe" } else { "" };
  let archive_path = temp_dir.join(EXE_NAME).with_extension("zip");
  let exe_path = temp_dir.join(EXE_NAME).with_extension(exe_ext);
  assert!(!exe_path.exists());

  let archive_ext = Path::new(&*ARCHIVE_NAME)
    .extension()
    .and_then(|ext| ext.to_str())
    .unwrap();
  let unpack_status = match archive_ext {
    "zip" if cfg!(windows) => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("powershell.exe")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-Command")
        .arg(
          "& {
            param($Path, $DestinationPath)
            trap { $host.ui.WriteErrorLine($_.Exception); exit 1 }
            Add-Type -AssemblyName System.IO.Compression.FileSystem
            [System.IO.Compression.ZipFile]::ExtractToDirectory(
              $Path,
              $DestinationPath
            );
          }",
        )
        .arg("-Path")
        .arg(format!("'{}'", &archive_path.to_str().unwrap()))
        .arg("-DestinationPath")
        .arg(format!("'{}'", &temp_dir.to_str().unwrap()))
        .spawn()?
        .wait()?
    }
    "zip" => {
      fs::write(&archive_path, &archive_data)?;
      Command::new("unzip")
        .current_dir(&temp_dir)
        .arg(archive_path)
        .spawn()
        .map_err(|err| {
          if err.kind() == std::io::ErrorKind::NotFound {
            std::io::Error::new(
              std::io::ErrorKind::NotFound,
              "`unzip` was not found on your PATH, please install `unzip`",
            )
          } else {
            err
          }
        })?
        .wait()?
    }
    ext => panic!("Unsupported archive type: '{}'", ext),
  };
  assert!(unpack_status.success());
  assert!(exe_path.exists());
  Ok(exe_path)
}

fn replace_exe(new: &Path, old: &Path) -> Result<(), std::io::Error> {
  if cfg!(windows) {
    // On windows you cannot replace the currently running executable.
    // so first we rename it to deno.old.exe
    fs::rename(old, old.with_extension("old.exe"))?;
  } else {
    fs::remove_file(old)?;
  }
  // Windows cannot rename files across device boundaries, so if rename fails,
  // we try again with copy.
  fs::rename(new, old).or_else(|_| fs::copy(new, old).map(|_| ()))?;
  Ok(())
}

fn check_exe(exe_path: &Path) -> Result<(), AnyError> {
  let output = Command::new(exe_path)
    .arg("-V")
    .stderr(std::process::Stdio::inherit())
    .output()?;
  assert!(output.status.success());
  Ok(())
}

#[derive(Debug)]
struct CheckVersionFile {
  pub last_prompt: chrono::DateTime<chrono::Utc>,
  pub last_checked: chrono::DateTime<chrono::Utc>,
  pub latest_version: String,
}

impl CheckVersionFile {
  pub fn parse(content: String) -> Option<Self> {
    let split_content = content.split('!').collect::<Vec<_>>();

    if split_content.len() != 3 {
      return None;
    }

    let latest_version = split_content[2].trim().to_owned();
    if latest_version.is_empty() {
      return None;
    }

    let last_prompt = chrono::DateTime::parse_from_rfc3339(split_content[0])
      .map(|dt| dt.with_timezone(&chrono::Utc))
      .ok()?;
    let last_checked = chrono::DateTime::parse_from_rfc3339(split_content[1])
      .map(|dt| dt.with_timezone(&chrono::Utc))
      .ok()?;

    Some(CheckVersionFile {
      last_prompt,
      last_checked,
      latest_version,
    })
  }

  fn serialize(&self) -> String {
    format!(
      "{}!{}!{}",
      self.last_prompt.to_rfc3339(),
      self.last_checked.to_rfc3339(),
      self.latest_version
    )
  }

  fn with_last_prompt(self, dt: chrono::DateTime<chrono::Utc>) -> Self {
    Self {
      last_prompt: dt,
      ..self
    }
  }
}

#[cfg(test)]
mod test {
  use std::sync::Arc;

  use deno_core::parking_lot::Mutex;

  use super::*;

  #[test]
  fn test_parse_upgrade_check_file() {
    let file = CheckVersionFile::parse(
      "2020-01-01T00:00:00+00:00!2020-01-01T00:00:00+00:00!1.2.3".to_string(),
    )
    .unwrap();
    assert_eq!(
      file.last_prompt.to_rfc3339(),
      "2020-01-01T00:00:00+00:00".to_string()
    );
    assert_eq!(
      file.last_checked.to_rfc3339(),
      "2020-01-01T00:00:00+00:00".to_string()
    );
    assert_eq!(file.latest_version, "1.2.3".to_string());

    let result =
      CheckVersionFile::parse("2020-01-01T00:00:00+00:00!".to_string());
    assert!(result.is_none());

    let result = CheckVersionFile::parse("garbage!test".to_string());
    assert!(result.is_none());

    let result = CheckVersionFile::parse("test".to_string());
    assert!(result.is_none());
  }

  #[test]
  fn test_serialize_upgrade_check_file() {
    let file = CheckVersionFile {
      last_prompt: chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc),
      last_checked: chrono::DateTime::parse_from_rfc3339(
        "2020-01-01T00:00:00Z",
      )
      .unwrap()
      .with_timezone(&chrono::Utc),
      latest_version: "1.2.3".to_string(),
    };
    assert_eq!(
      file.serialize(),
      "2020-01-01T00:00:00+00:00!2020-01-01T00:00:00+00:00!1.2.3"
    );
  }

  #[derive(Clone)]
  struct TestUpdateCheckerEnvironment {
    file_text: Arc<Mutex<String>>,
    current_version: Arc<Mutex<String>>,
    latest_version: Arc<Mutex<Result<String, String>>>,
    time: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,
  }

  impl TestUpdateCheckerEnvironment {
    pub fn new() -> Self {
      Self {
        file_text: Default::default(),
        current_version: Default::default(),
        latest_version: Arc::new(Mutex::new(Ok("".to_string()))),
        time: Arc::new(Mutex::new(chrono::Utc::now())),
      }
    }

    pub fn add_hours(&self, hours: i64) {
      let mut time = self.time.lock();
      *time = time
        .checked_add_signed(chrono::Duration::hours(hours))
        .unwrap();
    }

    pub fn set_file_text(&self, text: &str) {
      *self.file_text.lock() = text.to_string();
    }

    pub fn set_current_version(&self, version: &str) {
      *self.current_version.lock() = version.to_string();
    }

    pub fn set_latest_version(&self, version: &str) {
      *self.latest_version.lock() = Ok(version.to_string());
    }

    pub fn set_latest_version_err(&self, err: &str) {
      *self.latest_version.lock() = Err(err.to_string());
    }
  }

  impl UpdateCheckerEnvironment for TestUpdateCheckerEnvironment {
    fn latest_version(&self) -> BoxFuture<'static, Result<String, AnyError>> {
      let env = self.clone();
      async move {
        match env.latest_version.lock().clone() {
          Ok(result) => Ok(result),
          Err(err) => bail!("{}", err),
        }
      }
      .boxed()
    }

    fn current_version(&self) -> Cow<str> {
      Cow::Owned(self.current_version.lock().clone())
    }

    fn read_check_file(&self) -> String {
      self.file_text.lock().clone()
    }

    fn write_check_file(&self, text: &str) {
      self.set_file_text(text);
    }

    fn current_time(&self) -> chrono::DateTime<chrono::Utc> {
      *self.time.lock()
    }
  }

  #[tokio::test]
  async fn test_update_checker() {
    let env = TestUpdateCheckerEnvironment::new();
    env.set_current_version("1.0.0");
    env.set_latest_version("1.1.0");
    let checker = UpdateChecker::new(env.clone());

    // no version, so we should check, but not prompt
    assert!(checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), None);

    // store the latest version
    fetch_and_store_latest_version(&env).await;

    // reload
    let checker = UpdateChecker::new(env.clone());

    // should not check for latest version because we just did
    assert!(!checker.should_check_for_new_version());
    // but should prompt
    assert_eq!(checker.should_prompt(), Some("1.1.0".to_string()));

    // fast forward an hour and bump the latest version
    env.add_hours(1);
    env.set_latest_version("1.2.0");
    assert!(!checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), Some("1.1.0".to_string()));

    // fast forward again and it should check for a newer version
    env.add_hours(UPGRADE_CHECK_INTERVAL);
    assert!(checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), Some("1.1.0".to_string()));

    fetch_and_store_latest_version(&env).await;

    // reload and store that we prompted
    let checker = UpdateChecker::new(env.clone());
    assert!(!checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), Some("1.2.0".to_string()));
    checker.store_prompted();

    // reload and it should now say not to prompt
    let checker = UpdateChecker::new(env.clone());
    assert!(!checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), None);

    // but if we fast forward past the upgrade interval it should prompt again
    env.add_hours(UPGRADE_CHECK_INTERVAL + 1);
    assert!(checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), Some("1.2.0".to_string()));

    // upgrade the version and it should stop prompting
    env.set_current_version("1.2.0");
    assert!(checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), None);

    // now try failing when fetching the latest version
    env.add_hours(UPGRADE_CHECK_INTERVAL + 1);
    env.set_latest_version_err("Failed");
    env.set_latest_version("1.3.0");

    // this will silently fail
    fetch_and_store_latest_version(&env).await;
    assert!(checker.should_check_for_new_version());
    assert_eq!(checker.should_prompt(), None);
  }
}
