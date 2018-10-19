extern crate atty;

use std::io;
use std::io::Read;
use errors::DenoResult;
use errors::permission_denied;

#[derive(Debug, Default, PartialEq)]
pub struct DenoPermissions {
  allow_write: bool,
  allow_net: bool,
  allow_env: bool,
}

impl DenoPermissions {
  pub fn check_write(&self, filename: &str) -> DenoResult<()> {
    if self.allow_write { return Ok(()) };
    // TODO get location (where access occurred)
    permission_prompt(format!("Deno requests write access to \"{}\".", filename))
  }

  pub fn check_net(&self, domain_name: &str) -> DenoResult<()> {
    if self.allow_net { return Ok(()) };
    // TODO get location (where access occurred)
    permission_prompt(format!("Deno requests network access to \"{}\".", domain_name))
  }

  pub fn check_env(&self) -> DenoResult<()> {
    if self.allow_env { return Ok(()) };
    // TODO get location (where access occurred)
    permission_prompt("Deno requests access to environment variables.".to_string())
  }

  pub fn allow_write(&mut self, b: bool) -> () {
    self.allow_write = b;
  }

  pub fn allow_net(&mut self, b: bool) -> () {
    self.allow_net = b;
  }

  pub fn allow_env(&mut self, b: bool) -> () {
    self.allow_env = b;
  }
}

fn permission_prompt(message: String) -> DenoResult<()> {
  if !atty::is(atty::Stream::Stdin) || !atty::is(atty::Stream::Stderr) { return Err(permission_denied()) };
  // print to stderr so that if deno is > to a file this is still displayed.
  eprint!("{} Grant? yN", message);
  let mut buf = vec![0u8; 1];
  io::stdin().read_exact(&mut buf)?;
  let input = buf[0];
  let is_yes = input == 'y' as u8 || input == 'Y' as u8;
  if is_yes { Ok(()) } else { Err(permission_denied()) }
}