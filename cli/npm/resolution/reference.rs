use deno_ast::ModuleSpecifier;
use deno_core::anyhow::Context;
use deno_core::error::generic_error;
use deno_core::error::AnyError;
use serde::Deserialize;
use serde::Serialize;

use crate::semver::NpmVersionMatcher;
use crate::semver::Version;
use crate::semver::VersionReq;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpmPackageReference {
  pub req: NpmPackageReq,
  pub sub_path: Option<String>,
}

impl NpmPackageReference {
  pub fn from_specifier(
    specifier: &ModuleSpecifier,
  ) -> Result<NpmPackageReference, AnyError> {
    Self::from_str(specifier.as_str())
  }

  pub fn from_str(specifier: &str) -> Result<NpmPackageReference, AnyError> {
    let original_text = specifier;
    let specifier = match specifier.strip_prefix("npm:") {
      Some(s) => {
        // Strip leading slash, which might come from import map
        s.strip_prefix('/').unwrap_or(s)
      }
      None => {
        // don't allocate a string here and instead use a static string
        // because this is hit a lot when a url is not an npm specifier
        return Err(generic_error("Not an npm specifier"));
      }
    };
    let parts = specifier.split('/').collect::<Vec<_>>();
    let name_part_len = if specifier.starts_with('@') { 2 } else { 1 };
    if parts.len() < name_part_len {
      return Err(generic_error(format!("Not a valid package: {specifier}")));
    }
    let name_parts = &parts[0..name_part_len];
    let last_name_part = &name_parts[name_part_len - 1];
    let (name, version_req) = if let Some(at_index) = last_name_part.rfind('@')
    {
      let version = &last_name_part[at_index + 1..];
      let last_name_part = &last_name_part[..at_index];
      let version_req = VersionReq::parse_from_specifier(version)
        .with_context(|| "Invalid version requirement.")?;
      let name = if name_part_len == 1 {
        last_name_part.to_string()
      } else {
        format!("{}/{}", name_parts[0], last_name_part)
      };
      (name, Some(version_req))
    } else {
      (name_parts.join("/"), None)
    };
    let sub_path = if parts.len() == name_parts.len() {
      None
    } else {
      let sub_path = parts[name_part_len..].join("/");
      if sub_path.is_empty() {
        None
      } else {
        Some(sub_path)
      }
    };

    if let Some(sub_path) = &sub_path {
      if let Some(at_index) = sub_path.rfind('@') {
        let (new_sub_path, version) = sub_path.split_at(at_index);
        let msg = format!(
          "Invalid package specifier 'npm:{name}/{sub_path}'. Did you mean to write 'npm:{name}{version}/{new_sub_path}'?"
        );
        return Err(generic_error(msg));
      }
    }

    if name.is_empty() {
      let msg = format!(
        "Invalid npm specifier '{original_text}'. Did not contain a package name."
      );
      return Err(generic_error(msg));
    }

    Ok(NpmPackageReference {
      req: NpmPackageReq { name, version_req },
      sub_path,
    })
  }
}

impl std::fmt::Display for NpmPackageReference {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(sub_path) = &self.sub_path {
      write!(f, "npm:{}/{}", self.req, sub_path)
    } else {
      write!(f, "npm:{}", self.req)
    }
  }
}

#[derive(
  Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub struct NpmPackageReq {
  pub name: String,
  pub version_req: Option<VersionReq>,
}

impl std::fmt::Display for NpmPackageReq {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.version_req {
      Some(req) => write!(f, "{}@{}", self.name, req),
      None => write!(f, "{}", self.name),
    }
  }
}

impl NpmPackageReq {
  pub fn from_str(text: &str) -> Result<Self, AnyError> {
    // probably should do something more targeted in the future
    let reference = NpmPackageReference::from_str(&format!("npm:{text}"))?;
    Ok(reference.req)
  }
}

impl NpmVersionMatcher for NpmPackageReq {
  fn tag(&self) -> Option<&str> {
    match &self.version_req {
      Some(version_req) => version_req.tag(),
      None => Some("latest"),
    }
  }

  fn matches(&self, version: &Version) -> bool {
    match self.version_req.as_ref() {
      Some(req) => {
        assert_eq!(self.tag(), None);
        req.matches(version)
      }
      None => version.pre.is_empty(),
    }
  }

  fn version_text(&self) -> String {
    self
      .version_req
      .as_ref()
      .map(|v| format!("{v}"))
      .unwrap_or_else(|| "non-prerelease".to_string())
  }
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn parse_npm_package_ref() {
    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@1").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(VersionReq::parse_from_specifier("1").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: Some(VersionReq::parse_from_specifier("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@^1.2").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(VersionReq::parse_from_specifier("^1.2").unwrap()),
        },
        sub_path: None,
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:test@~1.1/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: Some(VersionReq::parse_from_specifier("~1.1").unwrap()),
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );

    assert_eq!(
      NpmPackageReference::from_str("npm:@package")
        .err()
        .unwrap()
        .to_string(),
      "Not a valid package: @package"
    );

    // should parse leading slash
    assert_eq!(
      NpmPackageReference::from_str("npm:/@package/test/sub_path").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "@package/test".to_string(),
          version_req: None,
        },
        sub_path: Some("sub_path".to_string()),
      }
    );
    assert_eq!(
      NpmPackageReference::from_str("npm:/test").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );
    assert_eq!(
      NpmPackageReference::from_str("npm:/test/").unwrap(),
      NpmPackageReference {
        req: NpmPackageReq {
          name: "test".to_string(),
          version_req: None,
        },
        sub_path: None,
      }
    );

    // should error for no name
    assert_eq!(
      NpmPackageReference::from_str("npm:/")
        .err()
        .unwrap()
        .to_string(),
      "Invalid npm specifier 'npm:/'. Did not contain a package name."
    );
    assert_eq!(
      NpmPackageReference::from_str("npm://test")
        .err()
        .unwrap()
        .to_string(),
      "Invalid npm specifier 'npm://test'. Did not contain a package name."
    );
  }
}
