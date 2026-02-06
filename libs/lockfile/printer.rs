// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;

use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::jsr::JsrDepPackageReq;
use deno_semver::package::PackageNv;
use serde::Serialize;

use crate::JsrPackageInfo;
use crate::LockfileContent;
use crate::LockfileLinkContent;
use crate::LockfilePackageJsonContent;
use crate::NpmPackageInfo;
use crate::WorkspaceConfigContent;
use crate::WorkspaceMemberConfigContent;

#[derive(Serialize)]
struct SerializedJsrPkg<'a> {
  integrity: &'a str,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  dependencies: Vec<StackString>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedNpmPkg<'a> {
  /// Will be `None` for patch packages.
  #[serde(skip_serializing_if = "Option::is_none")]
  integrity: Option<&'a str>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  dependencies: Vec<Cow<'a, str>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  optional_dependencies: Vec<Cow<'a, str>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  optional_peers: Vec<Cow<'a, str>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  os: Vec<SmallStackString>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  cpu: Vec<SmallStackString>,
  #[serde(skip_serializing_if = "is_false")]
  deprecated: bool,
  #[serde(skip_serializing_if = "is_false")]
  scripts: bool,
  #[serde(skip_serializing_if = "is_false")]
  bin: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  tarball: Option<&'a str>,
}

fn is_false(value: &bool) -> bool {
  !value
}

// WARNING: It's important to implement Ord/PartialOrd on the final
// normalized string so that sorting works according to the final
// output and so that's why this is used rather than JsrDepPackageReq
// directly.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct SerializedJsrDepPackageReq(StackString);

impl SerializedJsrDepPackageReq {
  pub fn new(dep_req: &JsrDepPackageReq) -> Self {
    Self(dep_req.to_string_normalized())
  }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedLockfilePackageJsonContent<'a> {
  #[serde(default)]
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub dependencies: Vec<SerializedJsrDepPackageReq>,
  /// npm overrides from the root package.json (only set for root)
  #[serde(default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub overrides: Option<&'a serde_json::Value>,
}

impl SerializedLockfilePackageJsonContent<'_> {
  pub fn is_empty(&self) -> bool {
    self.dependencies.is_empty() && self.overrides.is_none()
  }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedLockfileLinkContent {
  #[serde(default)]
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub dependencies: Vec<SerializedJsrDepPackageReq>,
  #[serde(default)]
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub optional_dependencies: Vec<SerializedJsrDepPackageReq>,
  #[serde(default)]
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub peer_dependencies: Vec<SerializedJsrDepPackageReq>,
  #[serde(default)]
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  pub peer_dependencies_meta: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedWorkspaceMemberConfigContent<'a> {
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[serde(default)]
  pub dependencies: Vec<SerializedJsrDepPackageReq>,
  #[serde(
    skip_serializing_if = "SerializedLockfilePackageJsonContent::is_empty"
  )]
  #[serde(default)]
  pub package_json: SerializedLockfilePackageJsonContent<'a>,
}

impl SerializedWorkspaceMemberConfigContent<'_> {
  pub fn is_empty(&self) -> bool {
    self.dependencies.is_empty() && self.package_json.is_empty()
  }
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SerializedWorkspaceConfigContent<'a> {
  #[serde(default, flatten)]
  pub root: SerializedWorkspaceMemberConfigContent<'a>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[serde(default)]
  pub members: BTreeMap<&'a str, SerializedWorkspaceMemberConfigContent<'a>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[serde(default)]
  pub links: BTreeMap<&'a str, SerializedLockfileLinkContent>,
}

impl SerializedWorkspaceConfigContent<'_> {
  pub fn is_empty(&self) -> bool {
    self.root.is_empty() && self.members.is_empty() && self.links.is_empty()
  }
}

#[derive(Serialize)]
struct LockfileV5<'a> {
  // order these based on auditability
  version: &'static str,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  specifiers: BTreeMap<SerializedJsrDepPackageReq, &'a str>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  jsr: BTreeMap<&'a PackageNv, SerializedJsrPkg<'a>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  npm: BTreeMap<&'a str, SerializedNpmPkg<'a>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  redirects: &'a BTreeMap<String, String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  remote: &'a BTreeMap<String, String>,
  #[serde(skip_serializing_if = "SerializedWorkspaceConfigContent::is_empty")]
  workspace: SerializedWorkspaceConfigContent<'a>,
}

pub fn print_v5_content(content: &LockfileContent) -> String {
  fn handle_jsr<'a>(
    jsr: &'a BTreeMap<PackageNv, JsrPackageInfo>,
    specifiers: &HashMap<JsrDepPackageReq, SmallStackString>,
  ) -> BTreeMap<&'a PackageNv, SerializedJsrPkg<'a>> {
    fn create_had_multiple_specifiers_map(
      specifiers: &HashMap<JsrDepPackageReq, SmallStackString>,
    ) -> HashMap<&str, bool> {
      let mut had_multiple_specifiers: HashMap<&str, bool> =
        HashMap::with_capacity(specifiers.len());
      for dep in specifiers.keys() {
        had_multiple_specifiers
          .entry(&dep.req.name)
          .and_modify(|v| *v = true)
          .or_default();
      }
      had_multiple_specifiers
    }

    let pkg_had_multiple_specifiers =
      create_had_multiple_specifiers_map(specifiers);

    jsr
      .iter()
      .map(|(key, value)| {
        (
          key,
          SerializedJsrPkg {
            integrity: &value.integrity,
            dependencies: {
              let mut dependencies = value
                .dependencies
                .iter()
                .map(|dep| {
                  let has_single_specifier = pkg_had_multiple_specifiers
                    .get(dep.req.name.as_str())
                    .map(|had_multiple| !had_multiple)
                    .unwrap_or(false);
                  if has_single_specifier {
                    let mut stack_string = StackString::with_capacity(
                      dep.kind.scheme_with_colon().len() + dep.req.name.len(),
                    );
                    stack_string.push_str(dep.kind.scheme_with_colon());
                    stack_string.push_str(dep.req.name.as_str());
                    stack_string
                  } else {
                    dep.to_string_normalized()
                  }
                })
                .collect::<Vec<_>>();
              dependencies.sort();
              dependencies
            },
          },
        )
      })
      .collect()
  }

  fn handle_npm(
    npm: &BTreeMap<StackString, NpmPackageInfo>,
  ) -> BTreeMap<&'_ str, SerializedNpmPkg<'_>> {
    fn extract_nv_from_id(value: &str) -> Option<(&str, &str)> {
      if value.is_empty() {
        return None;
      }
      let at_index = value[1..].find('@').map(|i| i + 1)?;
      let name = &value[..at_index];
      let version = &value[at_index + 1..];
      Some((name, version))
    }

    let mut pkg_had_multiple_versions: HashMap<&str, bool> =
      HashMap::with_capacity(npm.len());
    for id in npm.keys() {
      let Some((name, _)) = extract_nv_from_id(id) else {
        continue; // corrupt
      };
      pkg_had_multiple_versions
        .entry(name)
        .and_modify(|v| *v = true)
        .or_default();
    }

    fn handle_deps<'a>(
      deps: &'a BTreeMap<StackString, StackString>,
      pkg_had_multiple_versions: &HashMap<&str, bool>,
    ) -> Vec<Cow<'a, str>> {
      deps
        .iter()
        .filter_map(|(key, id)| {
          let (name, version) = extract_nv_from_id(id)?;
          if name == key {
            let has_single_version = pkg_had_multiple_versions
              .get(name)
              .map(|had_multiple| !had_multiple)
              .unwrap_or(false);
            if has_single_version {
              Some(Cow::Borrowed(name))
            } else {
              Some(Cow::Borrowed(id))
            }
          } else {
            Some(Cow::Owned(format!("{}@npm:{}@{}", key, name, version)))
          }
        })
        .collect::<Vec<_>>()
    }

    npm
      .iter()
      .map(|(key, value)| {
        let dependencies =
          handle_deps(&value.dependencies, &pkg_had_multiple_versions);
        let optional_dependencies =
          handle_deps(&value.optional_dependencies, &pkg_had_multiple_versions);
        let optional_peers =
          handle_deps(&value.optional_peers, &pkg_had_multiple_versions);
        (
          key.as_str(),
          SerializedNpmPkg {
            integrity: value.integrity.as_deref(),
            dependencies,
            optional_dependencies,
            optional_peers,
            os: value.os.clone(),
            cpu: value.cpu.clone(),
            tarball: value.tarball.as_deref(),
            deprecated: value.deprecated,
            scripts: value.scripts,
            bin: value.bin,
          },
        )
      })
      .collect()
  }

  fn handle_pkg_json_content<'a>(
    content: &LockfilePackageJsonContent,
    npm_overrides: Option<&'a serde_json::Value>,
  ) -> SerializedLockfilePackageJsonContent<'a> {
    SerializedLockfilePackageJsonContent {
      dependencies: sort_deps(&content.dependencies),
      overrides: npm_overrides,
    }
  }

  fn handle_workspace_member<'a>(
    member: &WorkspaceMemberConfigContent,
    npm_overrides: Option<&'a serde_json::Value>,
  ) -> SerializedWorkspaceMemberConfigContent<'a> {
    SerializedWorkspaceMemberConfigContent {
      dependencies: sort_deps(&member.dependencies),
      package_json: handle_pkg_json_content(
        &member.package_json,
        npm_overrides,
      ),
    }
  }

  fn handle_patch_content(
    content: &LockfileLinkContent,
  ) -> SerializedLockfileLinkContent {
    SerializedLockfileLinkContent {
      dependencies: sort_deps(&content.dependencies),
      optional_dependencies: sort_deps(&content.optional_dependencies),
      peer_dependencies: sort_deps(&content.peer_dependencies),
      peer_dependencies_meta: content
        .peer_dependencies_meta
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect(),
    }
  }

  fn sort_deps(
    deps: &HashSet<JsrDepPackageReq>,
  ) -> Vec<SerializedJsrDepPackageReq> {
    let mut dependencies = deps
      .iter()
      .map(SerializedJsrDepPackageReq::new)
      .collect::<Vec<_>>();
    dependencies.sort();
    dependencies
  }

  fn handle_workspace(
    content: &WorkspaceConfigContent,
  ) -> SerializedWorkspaceConfigContent<'_> {
    SerializedWorkspaceConfigContent {
      // pass npm_overrides only to root's packageJson section
      root: handle_workspace_member(
        &content.root,
        content.npm_overrides.as_ref(),
      ),
      members: content
        .members
        .iter()
        .map(|(key, value)| {
          (key.as_str(), handle_workspace_member(value, None))
        })
        .collect(),
      links: content
        .links
        .iter()
        .map(|(key, value)| (key.as_str(), handle_patch_content(value)))
        .collect(),
    }
  }

  // insert sorted
  let mut specifiers = BTreeMap::new();
  for (key, value) in &content.packages.specifiers {
    // insert a string to ensure proper sorting
    specifiers.insert(SerializedJsrDepPackageReq::new(key), value.as_str());
  }

  let lockfile = LockfileV5 {
    version: "5",
    specifiers,
    jsr: handle_jsr(&content.packages.jsr, &content.packages.specifiers),
    npm: handle_npm(&content.packages.npm),
    redirects: &content.redirects,
    remote: &content.remote,
    workspace: handle_workspace(&content.workspace),
  };
  let mut writer = Vec::with_capacity(1024);
  let mut serializer =
    serde_json::Serializer::with_formatter(&mut writer, Formatter::default());
  lockfile.serialize(&mut serializer).unwrap();
  String::from_utf8(writer).unwrap()
}

fn indent<W>(wr: &mut W, n: usize, s: &[u8]) -> io::Result<()>
where
  W: ?Sized + io::Write,
{
  for _ in 0..n {
    wr.write_all(s)?;
  }

  Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct Formatter<'a> {
  last_key: Option<String>,
  in_key: bool,
  current_indent: usize,
  indent: &'a [u8],
  has_value: bool,
}

impl Default for Formatter<'_> {
  fn default() -> Self {
    Self::new()
  }
}

impl Formatter<'_> {
  pub fn new() -> Self {
    Self {
      last_key: None,
      in_key: false,
      current_indent: 0,
      indent: b"  ",
      has_value: false,
    }
  }
}

// copied from serde_json::ser::PrettyFormatter
// except for the os and cpu handling
impl serde_json::ser::Formatter for Formatter<'_> {
  #[inline]
  fn write_string_fragment<W>(
    &mut self,
    writer: &mut W,
    fragment: &str,
  ) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    if self.in_key
      && let Some(last_key) = &mut self.last_key
    {
      last_key.push_str(fragment);
    }
    writer.write_all(fragment.as_bytes())
  }
  #[inline]
  fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    let mut should_indent = true;
    if let Some(last_key) = &self.last_key
      && (last_key == "os" || last_key == "cpu")
    {
      should_indent = false;
    }
    if should_indent {
      self.current_indent += 1;
    }
    self.has_value = false;
    writer.write_all(b"[")
  }

  #[inline]
  fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    let mut should_dedent = true;
    if let Some(last_key) = &self.last_key
      && (last_key == "os" || last_key == "cpu")
    {
      should_dedent = false;
    }
    if should_dedent {
      self.current_indent -= 1;
    }

    if self.has_value && should_dedent {
      writer.write_all(b"\n")?;
      indent(writer, self.current_indent, self.indent)?;
    }

    writer.write_all(b"]")
  }

  #[inline]
  fn begin_array_value<W>(
    &mut self,
    writer: &mut W,
    first: bool,
  ) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    if let Some(last_key) = &self.last_key
      && (last_key == "os" || last_key == "cpu")
    {
      if !first {
        writer.write_all(b", ")?;
      }

      return Ok(());
    }
    writer.write_all(if first { b"\n" } else { b",\n" })?;
    indent(writer, self.current_indent, self.indent)
  }

  #[inline]
  fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.has_value = true;
    Ok(())
  }

  #[inline]
  fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.current_indent += 1;
    self.has_value = false;
    writer.write_all(b"{")
  }

  #[inline]
  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.current_indent -= 1;

    if self.has_value {
      writer.write_all(b"\n")?;
      indent(writer, self.current_indent, self.indent)?;
    }

    writer.write_all(b"}")
  }

  #[inline]
  fn begin_object_key<W>(
    &mut self,
    writer: &mut W,
    first: bool,
  ) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.last_key = Some(String::new());
    self.in_key = true;
    writer.write_all(if first { b"\n" } else { b",\n" })?;
    indent(writer, self.current_indent, self.indent)
  }

  #[inline]
  fn end_object_key<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.in_key = false;
    Ok(())
  }

  #[inline]
  fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    writer.write_all(b": ")
  }

  #[inline]
  fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + io::Write,
  {
    self.has_value = true;
    self.last_key = None;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[must_use]
  pub fn trim_indent(mut text: &str) -> String {
    if text.starts_with('\n') {
      text = &text[1..];
    }
    // text = text.trim();
    let indent = text
      .lines()
      .filter(|l| !l.trim().is_empty())
      .map(|l| l.len() - l.trim_start().len())
      .min()
      .unwrap_or(0);

    text
      .split_inclusive('\n')
      .map(|l| {
        if l.len() <= indent {
          l.trim_start()
        } else {
          &l[indent..]
        }
      })
      .collect()
  }

  fn to_string_formatted(value: &serde_json::Value) -> String {
    let mut writer = Vec::new();
    let mut serializer =
      serde_json::Serializer::with_formatter(&mut writer, Formatter::default());
    value.serialize(&mut serializer).unwrap();
    String::from_utf8(writer).unwrap()
  }

  #[test]
  fn test_formatter() {
    let value = serde_json::json!({
      "os": ["darwin", "linux"],
      "cpu": ["x64", "arm64"]
    });
    let output = to_string_formatted(&value);
    let expected = trim_indent(
      r#"
      {
        "cpu": ["x64", "arm64"],
        "os": ["darwin", "linux"]
      }"#,
    );
    assert_eq!(output, expected);

    let value = serde_json::json!({
      "foo": {
        "bar": [
          {
            "os": ["darwin", "linux"],
            "cpu": ["x64"]
          },
          {
            "os\nos": ["foo"],
            "cpu\ncpu": ["bar"]
          }
        ]
      }
    });
    let output = to_string_formatted(&value);
    let expected = trim_indent(
      r#"
      {
        "foo": {
          "bar": [
            {
              "cpu": ["x64"],
              "os": ["darwin", "linux"]
            },
            {
              "cpu\ncpu": [
                "bar"
              ],
              "os\nos": [
                "foo"
              ]
            }
          ]
        }
      }"#,
    );
    assert_eq!(output, expected);
  }
}
