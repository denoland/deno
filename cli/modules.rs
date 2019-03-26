// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::ansi;
use crate::deno_dir::DenoDir;
use crate::msg;
use deno_core::Modules;
use std::collections::HashSet;
use std::fmt;

pub fn print_file_info(
  modules: &Modules,
  deno_dir: &DenoDir,
  filename: String,
) {
  let maybe_out = deno_dir.fetch_module_meta_data(&filename, ".", true);
  if maybe_out.is_err() {
    println!("{}", maybe_out.unwrap_err());
    return;
  }
  let out = maybe_out.unwrap();

  println!("{} {}", ansi::bold("local:".to_string()), &(out.filename));
  println!(
    "{} {}",
    ansi::bold("type:".to_string()),
    msg::enum_name_media_type(out.media_type)
  );
  if out.maybe_output_code_filename.is_some() {
    println!(
      "{} {}",
      ansi::bold("compiled:".to_string()),
      out.maybe_output_code_filename.as_ref().unwrap(),
    );
  }
  if out.maybe_source_map_filename.is_some() {
    println!(
      "{} {}",
      ansi::bold("map:".to_string()),
      out.maybe_source_map_filename.as_ref().unwrap()
    );
  }

  let deps = Deps::new(modules, &out.module_name);
  println!("{}{}", ansi::bold("deps:\n".to_string()), deps.name);
  if let Some(ref depsdeps) = deps.deps {
    for d in depsdeps {
      println!("{}", d);
    }
  }
}

pub struct Deps {
  pub name: String,
  pub deps: Option<Vec<Deps>>,
  prefix: String,
  is_last: bool,
}

impl Deps {
  pub fn new(modules: &Modules, module_name: &str) -> Deps {
    let mut seen = HashSet::new();
    Self::helper(
      &mut seen,
      "".to_string(),
      true,
      modules,
      module_name.to_string(),
    )
  }

  fn helper(
    seen: &mut HashSet<String>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    name: String,
  ) -> Deps {
    if seen.contains(&name) {
      Deps {
        name,
        prefix,
        deps: None,
        is_last,
      }
    } else {
      let name_ = name.clone();
      seen.insert(name);
      let child_names = modules.get_children2(&name_).unwrap();
      let child_count = child_names.iter().count();
      let deps = child_names
        .iter()
        .enumerate()
        .map(|(index, child_name)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');
          Self::helper(
            seen,
            new_prefix,
            new_is_last,
            modules,
            child_name.to_string(),
          )
        }).collect();
      Deps {
        name: name_,
        prefix,
        deps: Some(deps),
        is_last,
      }
    }
  }
}

impl fmt::Display for Deps {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let mut has_children = false;
    if let Some(ref deps) = self.deps {
      has_children = !deps.is_empty();
    }
    write!(
      f,
      "{}{}─{} {}",
      self.prefix,
      if self.is_last { "└" } else { "├" },
      if has_children { "┬" } else { "─" },
      self.name
    )?;

    if let Some(ref deps) = self.deps {
      for d in deps {
        write!(f, "\n{}", d)?;
      }
    }
    Ok(())
  }
}
