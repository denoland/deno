// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use deno_core::error::custom_error;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::serde_json::json;
use deno_core::serde_json::Value;
use deno_core::url::Url;
use deno_core::OpState;
use deno_core::ZeroCopyBuf;
use deno_runtime::permissions::PermissionState;
use deno_runtime::permissions::Permissions;
use deno_runtime::permissions::PermissionsOptions;
use serde::Deserialize;
use std::path::PathBuf;

pub fn init(rt: &mut deno_core::JsRuntime) {
  super::reg_sync(
    rt,
    "op_pledge_test_permissions",
    op_pledge_test_permissions,
  );
  super::reg_sync(
    rt,
    "op_restore_test_permissions",
    op_restore_test_permissions,
  );
}

fn test_permission_error(name: &str, info: Option<&str>) -> AnyError {
  custom_error(
    "PermissionDenied",
    format!(
      "Requires {}, run test again with the --allow-{} flag",
      format!(
        "{} access{}",
        name,
        info.map_or(String::new(), |info| { format!(" to {}", info) }),
      ),
      name
    ),
  )
}

struct RestoreTestPermissions(Permissions);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PledgeTestPermissionsArgs {
  read: Option<Vec<String>>,
  write: Option<Vec<String>>,
  net: Option<Vec<String>>,
  env: Option<Vec<String>>,
  run: Option<Vec<String>>,
  hrtime: Option<bool>,
  plugin: Option<bool>,
}

pub fn op_pledge_test_permissions(
  state: &mut OpState,
  args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let args: PledgeTestPermissionsArgs = serde_json::from_value(args)?;

  let mut permissions = state.borrow::<Permissions>().clone();
  state
    .put::<RestoreTestPermissions>(RestoreTestPermissions(permissions.clone()));

  let allow_read = if let Some(paths) = args.read {
    let mut allow_read = Vec::new();

    if paths.is_empty() {
      if permissions.read.request(None) != PermissionState::Granted {
        return Err(test_permission_error("read", None));
      }
    } else {
      for path in paths {
        let path = PathBuf::from(path);
        if permissions.read.request(Some(&path)) != PermissionState::Granted {
          return Err(test_permission_error("read", None));
        }

        allow_read.push(path)
      }
    }

    Some(allow_read)
  } else {
    None
  };

  let allow_write = if let Some(paths) = args.write {
    let mut allow_write = Vec::new();

    if paths.is_empty() {
      if permissions.write.request(None) != PermissionState::Granted {
        return Err(test_permission_error("write", None));
      }
    } else {
      for path in paths {
        let path = PathBuf::from(path);
        if permissions.write.request(Some(&path)) != PermissionState::Granted {
          return Err(test_permission_error("write", None));
        }

        allow_write.push(path)
      }
    }

    Some(allow_write)
  } else {
    None
  };

  let allow_net = if let Some(hosts) = args.net {
    let mut allow_net = Vec::new();

    if hosts.is_empty() {
      if permissions.net.request::<String>(None) != PermissionState::Granted {
        return Err(test_permission_error("net", None));
      }
    } else {
      for host in hosts {
        let url = Url::parse(&format!("http://{}", host))?;
        let hostname = url.host_str().unwrap().to_string();
        let port = url.port();

        if permissions.net.request(Some(&(&hostname, port)))
          != PermissionState::Granted
        {
          return Err(test_permission_error("net", None));
        }

        allow_net.push(host);
      }
    }

    Some(allow_net)
  } else {
    None
  };

  let allow_env = if let Some(names) = args.env {
    let mut allow_env = Vec::new();

    if names.is_empty() {
      if permissions.env.request(None) != PermissionState::Granted {
        return Err(test_permission_error("env", None));
      }
    } else {
      for name in names {
        if permissions.env.request(Some(&name)) != PermissionState::Granted {
          return Err(test_permission_error("env", None));
        }

        allow_env.push(name);
      }
    }

    Some(allow_env)
  } else {
    None
  };

  let allow_run = if let Some(commands) = args.run {
    let mut allow_run = Vec::new();

    if commands.is_empty() {
      if permissions.run.request(None) != PermissionState::Granted {
        return Err(test_permission_error("run", None));
      }
    } else {
      for command in commands {
        if permissions.run.request(Some(&command)) != PermissionState::Granted {
          return Err(test_permission_error("run", None));
        }

        allow_run.push(command);
      }
    }

    Some(allow_run)
  } else {
    None
  };

  let allow_hrtime = if args.hrtime.unwrap_or(false) {
    if permissions.hrtime.request() != PermissionState::Granted {
      return Err(test_permission_error("hrtime", None));
    }

    true
  } else {
    false
  };

  let allow_plugin = if args.plugin.unwrap_or(false) {
    if permissions.plugin.request() != PermissionState::Granted {
      return Err(test_permission_error("plugin", None));
    }

    true
  } else {
    false
  };

  let permissions = Permissions::from_options(&PermissionsOptions {
    allow_read,
    allow_write,
    allow_net,
    allow_env,
    allow_run,
    allow_hrtime,
    allow_plugin,
    prompt: false,
  });

  state.put::<Permissions>(permissions);

  Ok(json!({}))
}

pub fn op_restore_test_permissions(
  state: &mut OpState,
  _args: Value,
  _zero_copy: Option<ZeroCopyBuf>,
) -> Result<Value, AnyError> {
  let permissions =
    state.borrow::<RestoreTestPermissions>().clone().0.clone();
  state.put::<Permissions>(permissions);

  Ok(json!({}))
}
