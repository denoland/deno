// Copyright 2018-2026 the Deno authors. MIT license.

use std::collections::HashSet;
use std::path::PathBuf;

use deno_lockfile::Lockfile;
use deno_lockfile::SetWorkspaceConfigOptions;
use deno_lockfile::WorkspaceConfig;
use deno_lockfile::WorkspaceMemberConfig;
use deno_semver::jsr::JsrDepPackageReq;

#[test]
fn adding_workspace_does_not_cause_content_changes() {
  // should maintain the has_content_changed flag when lockfile empty
  {
    let mut lockfile = Lockfile::new_empty(PathBuf::from("./deno.lock"), true);

    assert!(!lockfile.has_content_changed);
    lockfile.set_workspace_config(SetWorkspaceConfigOptions {
      no_config: false,
      no_npm: false,
      config: WorkspaceConfig {
        root: WorkspaceMemberConfig {
          dependencies: HashSet::from([JsrDepPackageReq::from_str(
            "jsr:@scope/package",
          )
          .unwrap()]),
          package_json_deps: Default::default(),
        },
        members: Default::default(),
        links: Default::default(),
        npm_overrides: None,
      },
    });
    assert!(!lockfile.has_content_changed); // should not have changed
  }

  // should maintain has_content_changed flag when true and lockfile is empty
  {
    let mut lockfile = Lockfile::new_empty(PathBuf::from("./deno.lock"), true);
    lockfile.has_content_changed = true;
    lockfile.set_workspace_config(SetWorkspaceConfigOptions {
      no_config: false,
      no_npm: false,
      config: WorkspaceConfig {
        root: WorkspaceMemberConfig {
          dependencies: HashSet::from([JsrDepPackageReq::from_str(
            "jsr:@scope/package2",
          )
          .unwrap()]),
          package_json_deps: Default::default(),
        },
        members: Default::default(),
        links: Default::default(),
        npm_overrides: None,
      },
    });
    assert!(lockfile.has_content_changed);
  }

  // should not maintain the has_content_changed flag when lockfile is not empty
  {
    let mut lockfile = Lockfile::new_empty(PathBuf::from("./deno.lock"), true);
    lockfile
      .content
      .redirects
      .insert("a".to_string(), "b".to_string());

    assert!(!lockfile.has_content_changed);
    lockfile.set_workspace_config(SetWorkspaceConfigOptions {
      no_config: false,
      no_npm: false,
      config: WorkspaceConfig {
        root: WorkspaceMemberConfig {
          dependencies: HashSet::from([JsrDepPackageReq::from_str(
            "jsr:@scope/package",
          )
          .unwrap()]),
          package_json_deps: Default::default(),
        },
        members: Default::default(),
        links: Default::default(),
        npm_overrides: None,
      },
    });
    assert!(lockfile.has_content_changed); // should have changed since lockfile was not empty
  }
}

#[test]
fn npm_overrides_causes_content_change() {
  // setting npm_overrides should cause content change when lockfile not empty
  let mut lockfile = Lockfile::new_empty(PathBuf::from("./deno.lock"), true);
  lockfile
    .content
    .redirects
    .insert("a".to_string(), "b".to_string());

  assert!(!lockfile.has_content_changed);
  lockfile.set_workspace_config(SetWorkspaceConfigOptions {
    no_config: false,
    no_npm: false,
    config: WorkspaceConfig {
      root: Default::default(),
      members: Default::default(),
      links: Default::default(),
      npm_overrides: Some(serde_json::json!({
        "foo": "1.0.0"
      })),
    },
  });
  assert!(lockfile.has_content_changed);
}

#[test]
fn npm_overrides_serialized_in_package_json_section() {
  let mut lockfile = Lockfile::new_empty(PathBuf::from("./deno.lock"), true);
  lockfile.set_workspace_config(SetWorkspaceConfigOptions {
    no_config: false,
    no_npm: false,
    config: WorkspaceConfig {
      root: WorkspaceMemberConfig {
        dependencies: Default::default(),
        package_json_deps: HashSet::from([JsrDepPackageReq::from_str(
          "npm:foo@1.0.0",
        )
        .unwrap()]),
      },
      members: Default::default(),
      links: Default::default(),
      npm_overrides: Some(serde_json::json!({
        "bar": "2.0.0"
      })),
    },
  });

  // force content changed so we can serialize
  lockfile.has_content_changed = true;

  let output = lockfile.as_json_string();
  let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

  // verify overrides is in workspace.packageJson, not workspace directly
  assert!(
    parsed
      .get("workspace")
      .and_then(|w| w.get("packageJson"))
      .and_then(|pj| pj.get("overrides"))
      .is_some(),
    "overrides should be in workspace.packageJson section"
  );
  assert!(
    parsed
      .get("workspace")
      .and_then(|w| w.get("overrides"))
      .is_none(),
    "overrides should not be at workspace level"
  );

  // verify the actual value
  let overrides = parsed
    .get("workspace")
    .and_then(|w| w.get("packageJson"))
    .and_then(|pj| pj.get("overrides"))
    .unwrap();
  assert_eq!(overrides, &serde_json::json!({"bar": "2.0.0"}));
}
