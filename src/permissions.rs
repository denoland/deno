extern crate atty;

use flags::DenoFlags;

use errors::permission_denied;
use errors::DenoResult;
use std::io;

#[derive(Debug, Default, PartialEq)]
pub struct DenoPermissions {
  pub allow_write: bool,
  pub allow_net: bool,
  pub allow_env: bool,
}

impl DenoPermissions {
  pub fn new(flags: &DenoFlags) -> DenoPermissions {
    DenoPermissions {
      allow_write: flags.allow_write,
      allow_env: flags.allow_env,
      allow_net: flags.allow_net,
    }
  }

  pub fn check_write(&mut self, filename: &str) -> DenoResult<()> {
    if self.allow_write {
      return Ok(());
    };
    // TODO get location (where access occurred)
    let r = permission_prompt(&format!(
      "Deno requests write access to \"{}\".",
      filename
    ));;
    if r.is_ok() {
      self.allow_write = true;
    }
    r
  }

  pub fn check_net(&mut self, domain_name: &str) -> DenoResult<()> {
    if self.allow_net {
      return Ok(());
    };
    // TODO get location (where access occurred)
    let r = permission_prompt(&format!(
      "Deno requests network access to \"{}\".",
      domain_name
    ));
    if r.is_ok() {
      self.allow_net = true;
    }
    r
  }

  pub fn check_env(&mut self) -> DenoResult<()> {
    if self.allow_env {
      return Ok(());
    };
    // TODO get location (where access occurred)
    let r =
      permission_prompt(&"Deno requests access to environment variables.");
    if r.is_ok() {
      self.allow_env = true;
    }
    r
  }
}

fn permission_prompt(message: &str) -> DenoResult<()> {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) {
    return Err(permission_denied());
  };
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{} Grant? [yN] ", message);
  let mut input = String::new();
  let stdin = io::stdin();
  let _nread = stdin.read_line(&mut input)?;
  let ch = input.chars().next().unwrap();
  let is_yes = ch == 'y' || ch == 'Y';
  if is_yes {
    Ok(())
  } else {
    Err(permission_denied())
  }
}
