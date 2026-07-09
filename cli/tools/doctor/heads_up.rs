// Copyright 2018-2026 the Deno authors. MIT license.

use super::Note;

/// Report-only notes about Deno 3 changes whose designs are not final, so
/// no configuration pin exists yet. These never affect the `--check` exit
/// code.
pub fn notes() -> Vec<Note> {
  vec![
    Note {
      id: "permission-defaults",
      message:
        "Deno 3 changes some permission prompting defaults. There is no configuration to pin the current behavior yet; check the migration guide once available (https://docs.deno.com/runtime/help/deno-3)."
          .to_string(),
    },
    Note {
      id: "package-lock-json",
      message:
        "Deno 3 changes how package-lock.json files are taken into account in npm-compatible projects. There is no configuration to pin the current behavior yet; check the migration guide once available (https://docs.deno.com/runtime/help/deno-3)."
          .to_string(),
    },
  ]
}
