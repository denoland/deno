// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::path::Path;

pub fn resolve_cwd(
  initial_cwd: Option<&Path>,
) -> Result<Cow<'_, Path>, std::io::Error> {
  match initial_cwd {
    Some(initial_cwd) => Ok(Cow::Borrowed(initial_cwd)),
    // ok because the lint recommends using this method
    #[allow(clippy::disallowed_methods)]
    None => std::env::current_dir().map(Cow::Owned).map_err(|err| {
      std::io::Error::new(
        err.kind(),
        format!("could not read current working directory: {err}"),
      )
    }),
  }
}
