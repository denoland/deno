// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
use crate::ansi;
use crate::deno_dir::DenoDir;
use crate::msg;
use deno_core::deno_mod;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;

pub struct ModuleInfo {
  name: String,
  children: Vec<deno_mod>,
}

/// A collection of JS modules.
#[derive(Default)]
pub struct Modules {
  pub info: HashMap<deno_mod, ModuleInfo>,
  pub by_name: HashMap<String, deno_mod>,
}

impl Modules {
  pub fn new() -> Modules {
    Self {
      info: HashMap::new(),
      by_name: HashMap::new(),
    }
  }

  pub fn get_id(&self, name: &str) -> Option<deno_mod> {
    self.by_name.get(name).cloned()
  }

  pub fn get_children(&self, id: deno_mod) -> Option<&Vec<deno_mod>> {
    self.info.get(&id).map(|i| &i.children)
  }

  pub fn get_name(&self, id: deno_mod) -> Option<&String> {
    self.info.get(&id).map(|i| &i.name)
  }

  pub fn is_registered(&self, name: &str) -> bool {
    self.by_name.get(name).is_some()
  }

  pub fn register(&mut self, id: deno_mod, name: &str) {
    let name = String::from(name);
    debug!("register {}", name);
    self.by_name.insert(name.clone(), id);
    self.info.insert(
      id,
      ModuleInfo {
        name,
        children: Vec::new(),
      },
    );
  }

  pub fn resolve_cb(
    &mut self,
    deno_dir: &DenoDir,
    specifier: &str,
    referrer: deno_mod,
  ) -> deno_mod {
    debug!("resolve_cb {}", specifier);

    let maybe_info = self.info.get_mut(&referrer);
    if maybe_info.is_none() {
      debug!("cant find referrer {}", referrer);
      return 0;
    }
    let info = maybe_info.unwrap();
    let referrer_name = &info.name;
    let r = deno_dir.resolve_module(specifier, referrer_name);
    if let Err(err) = r {
      debug!("potentially swallowed err: {}", err);
      return 0;
    }
    let (name, _local_filename) = r.unwrap();

    if let Some(id) = self.by_name.get(&name) {
      let child_id = *id;
      info.children.push(child_id);
      return child_id;
    } else {
      return 0;
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
    let id = modules.get_id(module_name).unwrap();
    Self::helper(&mut seen, "".to_string(), true, modules, id)
  }

  fn helper(
    seen: &mut HashSet<deno_mod>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    id: deno_mod,
  ) -> Deps {
    let name = modules.get_name(id).unwrap().to_string();
    if seen.contains(&id) {
      Deps {
        name,
        prefix,
        deps: None,
        is_last,
      }
    } else {
      seen.insert(id);
      let child_ids = modules.get_children(id).unwrap();
      let child_count = child_ids.iter().count();
      let deps = child_ids
        .iter()
        .enumerate()
        .map(|(index, dep_id)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');
          Self::helper(seen, new_prefix, new_is_last, modules, *dep_id)
        }).collect();
      Deps {
        name,
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

pub fn print_file_info(
  modules: &Modules,
  deno_dir: &DenoDir,
  filename: String,
) {
  let maybe_out = deno_dir.fetch_module_meta_data(&filename, ".");
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
