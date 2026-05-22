// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

use deno_config::deno_json::NewestDependencyDate;
use deno_config::deno_json::NodeModulesDirMode;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_graph::packages::JsrPackageInfo;
use deno_path_util::ResolveUrlOrPathError;
use deno_runtime::WorkerExecutionMode;
use deno_runtime::deno_permissions::PermissionsContainer;

use crate::args::DeployFlags;
use crate::args::Flags;
use crate::factory::CliFactory;
use crate::ops;

pub async fn deploy(
  mut flags: Flags,
  deploy_flags: DeployFlags,
) -> Result<i32, AnyError> {
  flags.node_modules_dir = Some(NodeModulesDirMode::None);
  flags.no_lock = true;
  flags.minimum_dependency_age = Some(NewestDependencyDate::Disabled);
  if deploy_flags.sandbox {
    // SAFETY: only this subcommand is running, nothing else, so it's safe to set an env var.
    unsafe {
      std::env::set_var("DENO_DEPLOY_CLI_SANDBOX", "1");
    }
  }

  let mut factory = CliFactory::from_flags(Arc::new(flags));

  let specifier =
    if let Ok(specifier) = std::env::var("DENO_DEPLOY_CLI_SPECIFIER") {
      let specifier =
        Url::parse(&specifier).map_err(ResolveUrlOrPathError::UrlParse)?;
      if let Ok(path) = specifier.to_file_path() {
        factory.set_initial_cwd(path);
      }

      specifier
    } else {
      let registry_url = crate::args::jsr_url();
      let file = factory
        .file_fetcher()?
        .fetch_bypass_permissions(
          &registry_url.join("@deno/deploy/meta.json").unwrap(),
        )
        .await?;
      let info = serde_json::from_slice::<JsrPackageInfo>(&file.source)?;
      let latest_version = info
        .versions
        .keys()
        .max()
        .expect("expected @deno/deploy to be published");
      Url::parse(&format!("jsr:@deno/deploy@{latest_version}"))
        .map_err(ResolveUrlOrPathError::UrlParse)?
    };

  let worker_factory =
    Arc::new(factory.create_cli_main_worker_factory().await?);

  let mut worker = worker_factory
    .create_custom_worker(
      WorkerExecutionMode::Deploy,
      specifier,
      vec![],
      vec![],
      PermissionsContainer::allow_all(
        factory.permission_desc_parser()?.clone(),
      ),
      vec![ops::deploy::deno_deploy::init()],
      Default::default(),
      None,
    )
    .await?;

  Ok(worker.run().await?)
}

const DEPLOY_TOKEN_SERVICE: &str = "Deno Deploy Token";
const DEPLOY_TOKEN_USERNAME: &str = "Deno Deploy";

pub fn get_token_entry() -> Result<DeployTokenEntry, DeployTokenError> {
  DeployTokenEntry::new(DEPLOY_TOKEN_SERVICE, DEPLOY_TOKEN_USERNAME)
}

#[derive(Debug)]
pub enum DeployTokenError {
  NoEntry,
  Store(String),
}

impl std::fmt::Display for DeployTokenError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::NoEntry => f.write_str("deploy token not found"),
      Self::Store(message) => f.write_str(message),
    }
  }
}

impl std::error::Error for DeployTokenError {}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
pub struct DeployTokenEntry {
  service: &'static str,
  username: &'static str,
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
impl DeployTokenEntry {
  fn new(
    service: &'static str,
    username: &'static str,
  ) -> Result<Self, DeployTokenError> {
    Ok(Self { service, username })
  }

  pub fn get_password(&self) -> Result<String, DeployTokenError> {
    linux_secret_service::Entry::new(self.service, self.username).get_password()
  }

  pub fn set_password(&self, password: &str) -> Result<(), DeployTokenError> {
    linux_secret_service::Entry::new(self.service, self.username)
      .set_password(password)
  }

  pub fn delete_credential(&self) -> Result<(), DeployTokenError> {
    linux_secret_service::Entry::new(self.service, self.username)
      .delete_credential()
  }
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "freebsd",
  target_os = "openbsd"
)))]
pub struct DeployTokenEntry {
  inner: keyring::Entry,
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "freebsd",
  target_os = "openbsd"
)))]
impl DeployTokenEntry {
  fn new(service: &str, username: &str) -> Result<Self, DeployTokenError> {
    let inner =
      keyring::Entry::new(service, username).map_err(map_keyring_error)?;
    Ok(Self { inner })
  }

  pub fn get_password(&self) -> Result<String, DeployTokenError> {
    self.inner.get_password().map_err(map_keyring_error)
  }

  pub fn set_password(&self, password: &str) -> Result<(), DeployTokenError> {
    self.inner.set_password(password).map_err(map_keyring_error)
  }

  pub fn delete_credential(&self) -> Result<(), DeployTokenError> {
    self.inner.delete_credential().map_err(map_keyring_error)
  }
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "freebsd",
  target_os = "openbsd"
)))]
fn map_keyring_error(error: keyring::Error) -> DeployTokenError {
  match error {
    keyring::Error::NoEntry => DeployTokenError::NoEntry,
    error => DeployTokenError::Store(error.to_string()),
  }
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
mod linux_secret_service {
  use std::collections::HashMap;
  use std::sync::mpsc::Receiver;
  use std::sync::mpsc::Sender;
  use std::sync::mpsc::TryRecvError;
  use std::sync::mpsc::channel;
  use std::time::Duration;

  use dbus::Message;
  use dbus::arg::PropMap;
  use dbus::arg::ReadAll;
  use dbus::arg::RefArg;
  use dbus::arg::Variant;
  use dbus::blocking::Connection;
  use dbus::blocking::Proxy;
  use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;
  use dbus::channel::Token;
  use dbus::message::SignalArgs;
  use dbus::strings::Path;

  use super::DeployTokenError;

  const DEST: &str = "org.freedesktop.secrets";
  const SERVICE_PATH: &str = "/org/freedesktop/secrets";
  const SERVICE_IFACE: &str = "org.freedesktop.Secret.Service";
  const COLLECTION_IFACE: &str = "org.freedesktop.Secret.Collection";
  const ITEM_IFACE: &str = "org.freedesktop.Secret.Item";
  const PROMPT_IFACE: &str = "org.freedesktop.Secret.Prompt";
  const ITEM_LABEL: &str = "org.freedesktop.Secret.Item.Label";
  const ITEM_ATTRIBUTES: &str = "org.freedesktop.Secret.Item.Attributes";
  const DEFAULT_TARGET: &str = "default";
  const TEXT_PLAIN: &str = "text/plain";
  const TIMEOUT: Duration = Duration::from_millis(2000);
  type PromptResult = Result<PromptCompleted, DeployTokenError>;

  pub struct Entry {
    service: &'static str,
    username: &'static str,
  }

  impl Entry {
    pub fn new(service: &'static str, username: &'static str) -> Self {
      Self { service, username }
    }

    pub fn get_password(&self) -> Result<String, DeployTokenError> {
      let client = Client::connect()?;
      let item = self.find_unique_item(&client)?;
      let (_, _, password, _): (Path<'static>, Vec<u8>, Vec<u8>, String) =
        client.item_proxy(&item).method_call(
          ITEM_IFACE,
          "GetSecret",
          (client.session_path.clone(),),
        )?;
      String::from_utf8(password).map_err(|_| {
        DeployTokenError::Store("deploy token is not UTF-8".to_string())
      })
    }

    pub fn set_password(&self, password: &str) -> Result<(), DeployTokenError> {
      let client = Client::connect()?;
      match self.find_unique_item(&client) {
        Ok(item) => {
          let (): () = client.item_proxy(&item).method_call(
            ITEM_IFACE,
            "SetSecret",
            (client.secret(password.as_bytes()),),
          )?;
          Ok(())
        }
        Err(DeployTokenError::NoEntry) => {
          let collection = client.default_collection()?;
          let mut properties = PropMap::new();
          properties.insert(
            ITEM_LABEL.to_string(),
            Variant(Box::new(self.label()) as Box<dyn RefArg>),
          );
          properties.insert(
            ITEM_ATTRIBUTES.to_string(),
            Variant(Box::new(self.all_attributes()) as Box<dyn RefArg>),
          );
          let (item_path, prompt_path): (Path<'static>, Path<'static>) =
            client.collection_proxy(&collection).method_call(
              COLLECTION_IFACE,
              "CreateItem",
              (properties, client.secret(password.as_bytes()), true),
            )?;
          if item_path == root_path() {
            client.execute_prompt_for_path(&prompt_path).map(|_| ())
          } else {
            Ok(())
          }
        }
        Err(error) => Err(error),
      }
    }

    pub fn delete_credential(&self) -> Result<(), DeployTokenError> {
      let client = Client::connect()?;
      let item = self.find_unique_item(&client)?;
      let (prompt_path,): (Path<'static>,) = client
        .item_proxy(&item)
        .method_call(ITEM_IFACE, "Delete", ())?;
      if prompt_path == root_path() {
        Ok(())
      } else {
        client.execute_prompt(&prompt_path)
      }
    }

    fn find_unique_item(
      &self,
      client: &Client,
    ) -> Result<Path<'static>, DeployTokenError> {
      let (mut unlocked, mut locked): (Vec<Path<'static>>, Vec<Path<'static>>) =
        client.service_proxy().method_call(
          SERVICE_IFACE,
          "SearchItems",
          (self.search_attributes(false),),
        )?;
      if unlocked.is_empty() && locked.is_empty() {
        let collection = client.default_collection()?;
        let (legacy_unlocked,): (Vec<Path<'static>>,) =
          client.collection_proxy(&collection).method_call(
            COLLECTION_IFACE,
            "SearchItems",
            (self.search_attributes(true),),
          )?;
        unlocked = legacy_unlocked;
      }
      let count = unlocked.len() + locked.len();
      match count {
        0 => Err(DeployTokenError::NoEntry),
        1 => {
          if let Some(item) = unlocked.pop() {
            Ok(item)
          } else {
            client.unlock_paths(&locked)?;
            Ok(locked.pop().unwrap())
          }
        }
        _ => Err(DeployTokenError::Store(
          "multiple deploy tokens found in credential store".to_string(),
        )),
      }
    }

    fn all_attributes(&self) -> HashMap<String, String> {
      HashMap::from([
        ("service".to_string(), self.service.to_string()),
        ("username".to_string(), self.username.to_string()),
        ("target".to_string(), DEFAULT_TARGET.to_string()),
        ("application".to_string(), "rust-keyring".to_string()),
      ])
    }

    fn search_attributes(&self, omit_target: bool) -> HashMap<&str, &str> {
      let mut attributes =
        HashMap::from([("service", self.service), ("username", self.username)]);
      if !omit_target {
        attributes.insert("target", DEFAULT_TARGET);
      }
      attributes
    }

    fn label(&self) -> String {
      format!(
        "{}@{}:{} (keyring v3.6.3)",
        self.username, self.service, DEFAULT_TARGET
      )
    }
  }

  struct Client {
    connection: Connection,
    session_path: Path<'static>,
  }

  impl Client {
    fn connect() -> Result<Self, DeployTokenError> {
      let connection = Connection::new_session()?;
      let (_, session_path): (Variant<Box<dyn RefArg>>, Path<'static>) =
        connection
          .with_proxy(DEST, SERVICE_PATH, TIMEOUT)
          .method_call(
            SERVICE_IFACE,
            "OpenSession",
            ("plain", Variant(Box::new(String::new()) as Box<dyn RefArg>)),
          )?;
      Ok(Self {
        connection,
        session_path,
      })
    }

    fn service_proxy(&self) -> Proxy<'_, &Connection> {
      self.connection.with_proxy(DEST, SERVICE_PATH, TIMEOUT)
    }

    fn collection_proxy(&self, path: &Path<'static>) -> Proxy<'_, &Connection> {
      self.connection.with_proxy(DEST, path.clone(), TIMEOUT)
    }

    fn item_proxy(&self, path: &Path<'static>) -> Proxy<'_, &Connection> {
      self.connection.with_proxy(DEST, path.clone(), TIMEOUT)
    }

    fn prompt_proxy(&self, path: &Path<'static>) -> Proxy<'_, &Connection> {
      self.connection.with_proxy(DEST, path.clone(), TIMEOUT)
    }

    fn default_collection(&self) -> Result<Path<'static>, DeployTokenError> {
      let (path,): (Path<'static>,) = self.service_proxy().method_call(
        SERVICE_IFACE,
        "ReadAlias",
        ("default",),
      )?;
      if path == root_path() {
        Err(DeployTokenError::Store(
          "default Secret Service collection not found".to_string(),
        ))
      } else {
        self.ensure_collection_unlocked(&path)?;
        Ok(path)
      }
    }

    fn ensure_collection_unlocked(
      &self,
      path: &Path<'static>,
    ) -> Result<(), DeployTokenError> {
      let locked: bool = self
        .collection_proxy(path)
        .get(COLLECTION_IFACE, "Locked")?;
      if locked {
        self.unlock_paths(std::slice::from_ref(path))
      } else {
        Ok(())
      }
    }

    fn unlock_paths(
      &self,
      paths: &[Path<'static>],
    ) -> Result<(), DeployTokenError> {
      let (_, prompt_path): (Vec<Path<'static>>, Path<'static>) = self
        .service_proxy()
        .method_call(SERVICE_IFACE, "Unlock", (paths.to_vec(),))?;
      if prompt_path == root_path() {
        Ok(())
      } else {
        self.execute_prompt(&prompt_path)
      }
    }

    fn secret(
      &self,
      password: &[u8],
    ) -> (Path<'static>, Vec<u8>, Vec<u8>, &'static str) {
      (
        self.session_path.clone(),
        vec![],
        password.to_vec(),
        TEXT_PLAIN,
      )
    }

    fn execute_prompt_for_path(
      &self,
      path: &Path<'static>,
    ) -> Result<Path<'static>, DeployTokenError> {
      let completed = self.prompt(path)?;
      if completed.dismissed {
        Err(DeployTokenError::Store(
          "Secret Service prompt was dismissed".to_string(),
        ))
      } else if let Some(path) = prompt_result_path(&completed.result) {
        Ok(path)
      } else {
        Err(DeployTokenError::Store(
          "Secret Service prompt returned an invalid result".to_string(),
        ))
      }
    }

    fn execute_prompt(
      &self,
      path: &Path<'static>,
    ) -> Result<(), DeployTokenError> {
      let completed = self.prompt(path)?;
      if completed.dismissed {
        Err(DeployTokenError::Store(
          "Secret Service prompt was dismissed".to_string(),
        ))
      } else {
        Ok(())
      }
    }

    fn prompt(
      &self,
      path: &Path<'static>,
    ) -> Result<PromptCompleted, DeployTokenError> {
      let (tx, rx): (Sender<PromptResult>, Receiver<PromptResult>) = channel();
      let handler =
        move |signal: PromptCompleted, _: &Connection, _: &Message| {
          tx.send(Ok(signal)).unwrap();
          false
        };
      let proxy = self.prompt_proxy(path);
      let token: Token = proxy.match_signal(handler)?;
      let (): () = proxy.method_call(PROMPT_IFACE, "Prompt", ("",))?;
      let result = self.wait_for_prompt(rx);
      proxy.match_stop(token, true)?;
      result
    }

    fn wait_for_prompt(
      &self,
      rx: Receiver<PromptResult>,
    ) -> Result<PromptCompleted, DeployTokenError> {
      loop {
        match self.connection.process(Duration::from_millis(1000)) {
          Ok(false) => {}
          Ok(true) => match rx.try_recv() {
            Ok(result) => return result,
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
              return Err(DeployTokenError::Store(
                "Secret Service prompt was disconnected".to_string(),
              ));
            }
          },
          Err(error) => return Err(error.into()),
        }
      }
    }
  }

  #[derive(Debug)]
  struct PromptCompleted {
    dismissed: bool,
    result: Variant<Box<dyn RefArg + 'static>>,
  }

  impl dbus::arg::AppendAll for PromptCompleted {
    fn append(&self, i: &mut dbus::arg::IterAppend) {
      RefArg::append(&self.dismissed, i);
      RefArg::append(&self.result, i);
    }
  }

  impl ReadAll for PromptCompleted {
    fn read(
      i: &mut dbus::arg::Iter,
    ) -> Result<Self, dbus::arg::TypeMismatchError> {
      Ok(Self {
        dismissed: i.read()?,
        result: i.read()?,
      })
    }
  }

  impl SignalArgs for PromptCompleted {
    const NAME: &'static str = "Completed";
    const INTERFACE: &'static str = PROMPT_IFACE;
  }

  impl From<dbus::Error> for DeployTokenError {
    fn from(error: dbus::Error) -> Self {
      DeployTokenError::Store(format!("Secret Service error: {error}"))
    }
  }

  fn prompt_result_path(
    result: &Variant<Box<dyn RefArg + 'static>>,
  ) -> Option<Path<'static>> {
    result
      .0
      .as_str()
      .and_then(|s| Path::new(s).ok())
      .map(Path::into_static)
      .or_else(|| {
        result.0.as_iter().and_then(|mut i| {
          i.next()
            .and_then(|arg| arg.as_str().and_then(|s| Path::new(s).ok()))
            .map(Path::into_static)
        })
      })
  }

  fn root_path() -> Path<'static> {
    Path::new("/").unwrap().into_static()
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    #[test]
    fn uses_keyring_compatible_secret_service_attributes() {
      let entry = Entry::new("Deno Deploy Token", "Deno Deploy");
      assert_eq!(
        entry.search_attributes(false),
        HashMap::from([
          ("service", "Deno Deploy Token"),
          ("username", "Deno Deploy"),
          ("target", "default"),
        ])
      );
      assert_eq!(
        entry.search_attributes(true),
        HashMap::from([
          ("service", "Deno Deploy Token"),
          ("username", "Deno Deploy"),
        ])
      );
      assert_eq!(
        entry.all_attributes(),
        HashMap::from([
          ("service".to_string(), "Deno Deploy Token".to_string()),
          ("username".to_string(), "Deno Deploy".to_string()),
          ("target".to_string(), "default".to_string()),
          ("application".to_string(), "rust-keyring".to_string()),
        ])
      );
      assert_eq!(
        entry.label(),
        "Deno Deploy@Deno Deploy Token:default (keyring v3.6.3)"
      );
    }

    #[test]
    fn parses_object_path_prompt_result() {
      let path = Path::new("/org/freedesktop/secrets/item")
        .unwrap()
        .into_static();
      let result = Variant(Box::new(path.clone()) as Box<dyn RefArg>);
      assert_eq!(prompt_result_path(&result), Some(path));
    }
  }
}
