// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// Implementation note: one could imagine combining this module with Isolate to
// provide a more intuitive high-level API. However, due to the complexity
// inherent in asynchronous module loading, we would like the Isolate to remain
// small and simple for users who do not use modules or if they do can load them
// synchronously. The isolate.rs module should never depend on this module.

use crate::any_error::ErrBox;
use crate::isolate::ImportStream;
use crate::isolate::Isolate;
use crate::isolate::RecursiveLoadEvent as Event;
use crate::isolate::SourceCodeInfo;
use crate::libdeno::deno_dyn_import_id;
use crate::libdeno::deno_mod;
use crate::module_specifier::ModuleSpecifier;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

pub type SourceCodeInfoFuture =
  dyn Future<Output = Result<SourceCodeInfo, ErrBox>> + Send;

pub trait Loader: Send + Sync {
  /// Returns an absolute URL.
  /// When implementing an spec-complaint VM, this should be exactly the
  /// algorithm described here:
  /// https://html.spec.whatwg.org/multipage/webappapis.html#resolve-a-module-specifier
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    is_main: bool,
    is_dyn_import: bool,
  ) -> Result<ModuleSpecifier, ErrBox>;

  /// Given ModuleSpecifier, load its source code.
  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    maybe_referrer: Option<ModuleSpecifier>,
  ) -> Pin<Box<SourceCodeInfoFuture>>;
}

#[derive(Debug, Eq, PartialEq)]
enum Kind {
  Main,
  DynamicImport(deno_dyn_import_id),
}

#[derive(Debug, Eq, PartialEq)]
enum State {
  ResolveMain(String, Option<String>), // specifier, maybe code
  ResolveImport(String, String),       // specifier, referrer
  LoadingRoot,
  LoadingImports(deno_mod),
  Instantiated(deno_mod),
}

/// This future is used to implement parallel async module loading without
/// complicating the Isolate API.
/// TODO: RecursiveLoad desperately needs to be merged with Modules.
pub struct RecursiveLoad<L: Loader + Unpin> {
  kind: Kind,
  state: State,
  loader: L,
  modules: Arc<Mutex<Modules>>,
  pending: FuturesUnordered<Pin<Box<SourceCodeInfoFuture>>>,
  is_pending: HashSet<ModuleSpecifier>,
}

impl<L: Loader + Unpin> RecursiveLoad<L> {
  /// Starts a new parallel load of the given URL of the main module.
  pub fn main(
    specifier: &str,
    code: Option<String>,
    loader: L,
    modules: Arc<Mutex<Modules>>,
  ) -> Self {
    let kind = Kind::Main;
    let state = State::ResolveMain(specifier.to_owned(), code);
    Self::new(kind, state, loader, modules)
  }

  pub fn dynamic_import(
    id: deno_dyn_import_id,
    specifier: &str,
    referrer: &str,
    loader: L,
    modules: Arc<Mutex<Modules>>,
  ) -> Self {
    let kind = Kind::DynamicImport(id);
    let state = State::ResolveImport(specifier.to_owned(), referrer.to_owned());
    Self::new(kind, state, loader, modules)
  }

  pub fn dyn_import_id(&self) -> Option<deno_dyn_import_id> {
    match self.kind {
      Kind::Main => None,
      Kind::DynamicImport(id) => Some(id),
    }
  }

  fn new(
    kind: Kind,
    state: State,
    loader: L,
    modules: Arc<Mutex<Modules>>,
  ) -> Self {
    Self {
      kind,
      state,
      loader,
      modules,
      pending: FuturesUnordered::new(),
      is_pending: HashSet::new(),
    }
  }

  fn add_root(&mut self) -> Result<(), ErrBox> {
    let module_specifier = match self.state {
      State::ResolveMain(ref specifier, _) => self.loader.resolve(
        specifier,
        ".",
        true,
        self.dyn_import_id().is_some(),
      )?,
      State::ResolveImport(ref specifier, ref referrer) => self
        .loader
        .resolve(specifier, referrer, false, self.dyn_import_id().is_some())?,
      _ => unreachable!(),
    };

    // We deliberately do not check if this module is already present in the
    // module map. That's because the module map doesn't track whether a
    // a module's dependencies have been loaded and whether it's been
    // instantiated, so if we did find this module in the module map and used
    // its id, this could lead to a crash.
    //
    // For the time being code and metadata for a module specifier is fetched
    // multiple times, register() uses only the first result, and assigns the
    // same module id to all instances.
    //
    // TODO: this is very ugly. The module map and recursive loader should be
    // integrated into one thing.
    self
      .pending
      .push(self.loader.load(&module_specifier, None).boxed());
    self.state = State::LoadingRoot;

    Ok(())
  }

  fn add_import(
    &mut self,
    specifier: &str,
    referrer: &str,
    parent_id: deno_mod,
  ) -> Result<(), ErrBox> {
    let referrer_specifier = ModuleSpecifier::resolve_url(referrer)
      .expect("Referrer should be a valid specifier");
    let module_specifier = self.loader.resolve(
      specifier,
      referrer,
      false,
      self.dyn_import_id().is_some(),
    )?;
    let module_name = module_specifier.as_str();

    let mut modules = self.modules.lock().unwrap();

    modules.add_child(parent_id, module_name);

    if !modules.is_registered(module_name)
      && !self.is_pending.contains(&module_specifier)
    {
      let fut = self
        .loader
        .load(&module_specifier, Some(referrer_specifier.clone()));
      self.pending.push(fut.boxed());
      self.is_pending.insert(module_specifier);
    }

    Ok(())
  }

  /// Returns a future that resolves to the final module id of the root module.
  /// This future needs to take ownership of the isolate.
  pub fn get_future(
    self,
    isolate: Arc<Mutex<Isolate>>,
  ) -> impl Future<Output = Result<deno_mod, ErrBox>> {
    async move {
      let mut load = self;
      loop {
        let event = load.try_next().await?;
        match event.unwrap() {
          Event::Fetch(info) => {
            let mut isolate = isolate.lock().unwrap();
            load.register(info, &mut isolate)?;
          }
          Event::Instantiate(id) => return Ok(id),
        }
      }
    }
  }
}

impl<L: Loader + Unpin> ImportStream for RecursiveLoad<L> {
  // TODO: this should not be part of RecursiveLoad.
  fn register(
    &mut self,
    source_code_info: SourceCodeInfo,
    isolate: &mut Isolate,
  ) -> Result<(), ErrBox> {
    // #A There are 3 cases to handle at this moment:
    // 1. Source code resolved result have the same module name as requested
    //    and is not yet registered
    //     -> register
    // 2. Source code resolved result have a different name as requested:
    //   2a. The module with resolved module name has been registered
    //     -> alias
    //   2b. The module with resolved module name has not yet been registerd
    //     -> register & alias
    let SourceCodeInfo {
      code,
      module_url_specified,
      module_url_found,
    } = source_code_info;

    let is_main = self.kind == Kind::Main && self.state == State::LoadingRoot;

    let module_id = {
      let mut modules = self.modules.lock().unwrap();

      // If necessary, register an alias.
      if module_url_specified != module_url_found {
        modules.alias(&module_url_specified, &module_url_found);
      }

      match modules.get_id(&module_url_found) {
        // Module has already been registered.
        Some(id) => {
          debug!(
            "Already-registered module fetched again: {}",
            module_url_found
          );
          id
        }
        // Module not registered yet, do it now.
        None => {
          let id = isolate.mod_new(is_main, &module_url_found, &code)?;
          modules.register(id, &module_url_found);
          id
        }
      }
    };

    // Now we must iterate over all imports of the module and load them.
    let imports = isolate.mod_get_imports(module_id);
    for import in imports {
      self.add_import(&import, &module_url_found, module_id)?;
    }

    // If we just finished loading the root module, store the root module id.
    match self.state {
      State::LoadingRoot => self.state = State::LoadingImports(module_id),
      State::LoadingImports(..) => {}
      _ => unreachable!(),
    };

    // If all imports have been loaded, instantiate the root module.
    if self.pending.is_empty() {
      let root_id = match self.state {
        State::LoadingImports(mod_id) => mod_id,
        _ => unreachable!(),
      };

      let mut resolve_cb =
        |specifier: &str, referrer_id: deno_mod| -> deno_mod {
          let modules = self.modules.lock().unwrap();
          let referrer = modules.get_name(referrer_id).unwrap();
          match self.loader.resolve(
            specifier,
            &referrer,
            is_main,
            self.dyn_import_id().is_some(),
          ) {
            Ok(specifier) => modules.get_id(specifier.as_str()).unwrap_or(0),
            // We should have already resolved and Ready this module, so
            // resolve() will not fail this time.
            Err(..) => unreachable!(),
          }
        };
      isolate.mod_instantiate(root_id, &mut resolve_cb)?;

      self.state = State::Instantiated(root_id);
    }

    Ok(())
  }
}

impl<L: Loader + Unpin> Stream for RecursiveLoad<L> {
  type Item = Result<Event, ErrBox>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    match inner.state {
      State::ResolveMain(ref specifier, Some(ref code)) => {
        let module_specifier = inner.loader.resolve(
          specifier,
          ".",
          true,
          inner.dyn_import_id().is_some(),
        )?;
        let info = SourceCodeInfo {
          code: code.to_owned(),
          module_url_specified: module_specifier.to_string(),
          module_url_found: module_specifier.to_string(),
        };
        inner.state = State::LoadingRoot;
        Poll::Ready(Some(Ok(Event::Fetch(info))))
      }
      State::ResolveMain(..) | State::ResolveImport(..) => {
        if let Err(e) = inner.add_root() {
          return Poll::Ready(Some(Err(e)));
        }
        inner.try_poll_next_unpin(cx)
      }
      State::LoadingRoot | State::LoadingImports(..) => {
        match inner.pending.try_poll_next_unpin(cx)? {
          Poll::Ready(None) => unreachable!(),
          Poll::Ready(Some(info)) => Poll::Ready(Some(Ok(Event::Fetch(info)))),
          Poll::Pending => Poll::Pending,
        }
      }
      State::Instantiated(id) => Poll::Ready(Some(Ok(Event::Instantiate(id)))),
    }
  }
}

struct ModuleInfo {
  name: String,
  children: Vec<String>,
}

impl ModuleInfo {
  fn has_child(&self, child_name: &str) -> bool {
    for c in self.children.iter() {
      if c == child_name {
        return true;
      }
    }
    false
  }
}

/// A symbolic module entity.
enum SymbolicModule {
  /// This module is an alias to another module.
  /// This is useful such that multiple names could point to
  /// the same underlying module (particularly due to redirects).
  Alias(String),
  /// This module associates with a V8 module by id.
  Mod(deno_mod),
}

#[derive(Default)]
/// Alias-able module name map
struct ModuleNameMap {
  inner: HashMap<String, SymbolicModule>,
}

impl ModuleNameMap {
  pub fn new() -> Self {
    ModuleNameMap {
      inner: HashMap::new(),
    }
  }

  /// Get the id of a module.
  /// If this module is internally represented as an alias,
  /// follow the alias chain to get the final module id.
  pub fn get(&self, name: &str) -> Option<deno_mod> {
    let mut mod_name = name;
    loop {
      let cond = self.inner.get(mod_name);
      match cond {
        Some(SymbolicModule::Alias(target)) => {
          mod_name = target;
        }
        Some(SymbolicModule::Mod(mod_id)) => {
          return Some(*mod_id);
        }
        _ => {
          return None;
        }
      }
    }
  }

  /// Insert a name assocated module id.
  pub fn insert(&mut self, name: String, id: deno_mod) {
    self.inner.insert(name, SymbolicModule::Mod(id));
  }

  /// Create an alias to another module.
  pub fn alias(&mut self, name: String, target: String) {
    self.inner.insert(name, SymbolicModule::Alias(target));
  }

  /// Check if a name is an alias to another module.
  pub fn is_alias(&self, name: &str) -> bool {
    let cond = self.inner.get(name);
    match cond {
      Some(SymbolicModule::Alias(_)) => true,
      _ => false,
    }
  }
}

/// A collection of JS modules.
#[derive(Default)]
pub struct Modules {
  info: HashMap<deno_mod, ModuleInfo>,
  by_name: ModuleNameMap,
}

impl Modules {
  pub fn new() -> Modules {
    Self {
      info: HashMap::new(),
      by_name: ModuleNameMap::new(),
    }
  }

  pub fn get_id(&self, name: &str) -> Option<deno_mod> {
    self.by_name.get(name)
  }

  pub fn get_children(&self, id: deno_mod) -> Option<&Vec<String>> {
    self.info.get(&id).map(|i| &i.children)
  }

  pub fn get_children2(&self, name: &str) -> Option<&Vec<String>> {
    self.get_id(name).and_then(|id| self.get_children(id))
  }

  pub fn get_name(&self, id: deno_mod) -> Option<&String> {
    self.info.get(&id).map(|i| &i.name)
  }

  pub fn is_registered(&self, name: &str) -> bool {
    self.by_name.get(name).is_some()
  }

  pub fn add_child(&mut self, parent_id: deno_mod, child_name: &str) -> bool {
    self
      .info
      .get_mut(&parent_id)
      .map(move |i| {
        if !i.has_child(&child_name) {
          i.children.push(child_name.to_string());
        }
      })
      .is_some()
  }

  pub fn register(&mut self, id: deno_mod, name: &str) {
    let name = String::from(name);
    debug!("register_complete {}", name);

    self.by_name.insert(name.clone(), id);
    self.info.insert(
      id,
      ModuleInfo {
        name,
        children: Vec::new(),
      },
    );
  }

  pub fn alias(&mut self, name: &str, target: &str) {
    self.by_name.alias(name.to_owned(), target.to_owned());
  }

  pub fn is_alias(&self, name: &str) -> bool {
    self.by_name.is_alias(name)
  }

  pub fn deps(&self, url: &str) -> Option<Deps> {
    Deps::new(self, url)
  }
}

/// This is a tree structure representing the dependencies of a given module.
/// Use Modules::deps to construct it. The 'deps' member is None if this module
/// was already seen elsewher in the tree.
#[derive(Debug, PartialEq)]
pub struct Deps {
  pub name: String,
  pub deps: Option<Vec<Deps>>,
  prefix: String,
  is_last: bool,
}

impl Deps {
  fn new(modules: &Modules, module_name: &str) -> Option<Deps> {
    let mut seen = HashSet::new();
    Self::helper(&mut seen, "".to_string(), true, modules, module_name)
  }

  fn helper(
    seen: &mut HashSet<String>,
    prefix: String,
    is_last: bool,
    modules: &Modules,
    name: &str, // TODO(ry) rename url
  ) -> Option<Deps> {
    if seen.contains(name) {
      Some(Deps {
        name: name.to_string(),
        prefix,
        deps: None,
        is_last,
      })
    } else {
      let children = modules.get_children2(name)?;
      seen.insert(name.to_string());
      let child_count = children.len();
      let deps: Vec<Deps> = children
        .iter()
        .enumerate()
        .map(|(index, dep_name)| {
          let new_is_last = index == child_count - 1;
          let mut new_prefix = prefix.clone();
          new_prefix.push(if is_last { ' ' } else { '│' });
          new_prefix.push(' ');

          Self::helper(seen, new_prefix, new_is_last, modules, dep_name)
        })
        // If any of the children are missing, return None.
        .collect::<Option<_>>()?;

      Some(Deps {
        name: name.to_string(),
        prefix,
        deps: Some(deps),
        is_last,
      })
    }
  }

  pub fn to_json(&self) -> String {
    let mut children = "[".to_string();

    if let Some(ref deps) = self.deps {
      for d in deps {
        children.push_str(&d.to_json());
        if !d.is_last {
          children.push_str(",");
        }
      }
    }
    children.push_str("]");

    format!("[\"{}\",{}]", self.name, children)
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::isolate::js_check;
  use crate::isolate::tests::*;
  use futures::future::FutureExt;
  use futures::stream::StreamExt;
  use std::error::Error;
  use std::fmt;
  use std::future::Future;

  struct MockLoader {
    pub loads: Arc<Mutex<Vec<String>>>,
    pub isolate: Arc<Mutex<Isolate>>,
    pub modules: Arc<Mutex<Modules>>,
  }

  impl MockLoader {
    fn new() -> Self {
      let modules = Modules::new();
      let (isolate, _dispatch_count) = setup(Mode::Async);
      Self {
        loads: Arc::new(Mutex::new(Vec::new())),
        isolate: Arc::new(Mutex::new(isolate)),
        modules: Arc::new(Mutex::new(modules)),
      }
    }
  }

  fn mock_source_code(url: &str) -> Option<(&'static str, &'static str)> {
    // (code, real_module_name)
    let spec: Vec<&str> = url.split("file://").collect();
    match spec[1] {
      "/a.js" => Some((A_SRC, "file:///a.js")),
      "/b.js" => Some((B_SRC, "file:///b.js")),
      "/c.js" => Some((C_SRC, "file:///c.js")),
      "/d.js" => Some((D_SRC, "file:///d.js")),
      "/circular1.js" => Some((CIRCULAR1_SRC, "file:///circular1.js")),
      "/circular2.js" => Some((CIRCULAR2_SRC, "file:///circular2.js")),
      "/circular3.js" => Some((CIRCULAR3_SRC, "file:///circular3.js")),
      "/redirect1.js" => Some((REDIRECT1_SRC, "file:///redirect1.js")),
      // pretend redirect - real module name is different than one requested
      "/redirect2.js" => Some((REDIRECT2_SRC, "file:///dir/redirect2.js")),
      "/dir/redirect3.js" => Some((REDIRECT3_SRC, "file:///redirect3.js")),
      "/slow.js" => Some((SLOW_SRC, "file:///slow.js")),
      "/never_ready.js" => {
        Some(("should never be Ready", "file:///never_ready.js"))
      }
      "/main.js" => Some((MAIN_SRC, "file:///main.js")),
      "/bad_import.js" => Some((BAD_IMPORT_SRC, "file:///bad_import.js")),
      // deliberately empty code.
      "/main_with_code.js" => Some(("", "file:///main_with_code.js")),
      _ => None,
    }
  }

  #[derive(Debug, PartialEq)]
  enum MockError {
    ResolveErr,
    LoadErr,
  }

  impl fmt::Display for MockError {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
      unimplemented!()
    }
  }

  impl Error for MockError {
    fn cause(&self) -> Option<&dyn Error> {
      unimplemented!()
    }
  }

  struct DelayedSourceCodeFuture {
    url: String,
    counter: u32,
  }

  impl Future for DelayedSourceCodeFuture {
    type Output = Result<SourceCodeInfo, ErrBox>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
      let inner = self.get_mut();
      inner.counter += 1;
      if inner.url == "file:///never_ready.js" {
        return Poll::Pending;
      }
      if inner.url == "file:///slow.js" && inner.counter < 2 {
        // TODO(ry) Hopefully in the future we can remove current task
        // notification. See comment above run_in_task.
        cx.waker().wake_by_ref();
        return Poll::Pending;
      }
      match mock_source_code(&inner.url) {
        Some(src) => Poll::Ready(Ok(SourceCodeInfo {
          code: src.0.to_owned(),
          module_url_specified: inner.url.clone(),
          module_url_found: src.1.to_owned(),
        })),
        None => Poll::Ready(Err(MockError::LoadErr.into())),
      }
    }
  }

  impl Loader for MockLoader {
    fn resolve(
      &self,
      specifier: &str,
      referrer: &str,
      _is_root: bool,
      _is_dyn_import: bool,
    ) -> Result<ModuleSpecifier, ErrBox> {
      let referrer = if referrer == "." {
        "file:///"
      } else {
        referrer
      };

      eprintln!(">> RESOLVING, S: {}, R: {}", specifier, referrer);

      let output_specifier =
        match ModuleSpecifier::resolve_import(specifier, referrer) {
          Ok(specifier) => specifier,
          Err(..) => return Err(MockError::ResolveErr.into()),
        };

      if mock_source_code(&output_specifier.to_string()).is_some() {
        Ok(output_specifier)
      } else {
        Err(MockError::ResolveErr.into())
      }
    }

    fn load(
      &self,
      module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<ModuleSpecifier>,
    ) -> Pin<Box<SourceCodeInfoFuture>> {
      let mut loads = self.loads.lock().unwrap();
      loads.push(module_specifier.to_string());
      let url = module_specifier.to_string();
      DelayedSourceCodeFuture { url, counter: 0 }.boxed()
    }
  }

  const A_SRC: &str = r#"
    import { b } from "/b.js";
    import { c } from "/c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'file:///a.js') throw Error();
  "#;

  const B_SRC: &str = r#"
    import { c } from "/c.js";
    if (c() != 'c') throw Error();
    export function b() { return 'b'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///b.js') throw Error();
  "#;

  const C_SRC: &str = r#"
    import { d } from "/d.js";
    export function c() { return 'c'; }
    if (d() != 'd') throw Error();
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///c.js') throw Error();
  "#;

  const D_SRC: &str = r#"
    export function d() { return 'd'; }
    if (import.meta.main) throw Error();
    if (import.meta.url != 'file:///d.js') throw Error();
  "#;

  // TODO(ry) Sadly FuturesUnordered requires the current task to be set. So
  // even though we are only using poll() in these tests and not Tokio, we must
  // nevertheless run it in the tokio executor. Ideally run_in_task can be
  // removed in the future.
  use crate::isolate::tests::run_in_task;

  #[test]
  fn test_recursive_load() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let modules = loader.modules.clone();
      let modules_ = modules.clone();
      let isolate = loader.isolate.clone();
      let isolate_ = isolate.clone();
      let loads = loader.loads.clone();
      let mut recursive_load =
        RecursiveLoad::main("/a.js", None, loader, modules);

      let a_id = loop {
        match recursive_load.try_poll_next_unpin(&mut cx) {
          Poll::Ready(Some(Ok(Event::Fetch(info)))) => {
            let mut isolate = isolate.lock().unwrap();
            recursive_load.register(info, &mut isolate).unwrap();
          }
          Poll::Ready(Some(Ok(Event::Instantiate(id)))) => break id,
          _ => panic!("unexpected result"),
        };
      };

      let mut isolate = isolate_.lock().unwrap();
      js_check(isolate.mod_evaluate(a_id));

      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec![
          "file:///a.js",
          "file:///b.js",
          "file:///c.js",
          "file:///d.js"
        ]
      );

      let modules = modules_.lock().unwrap();

      assert_eq!(modules.get_id("file:///a.js"), Some(a_id));
      let b_id = modules.get_id("file:///b.js").unwrap();
      let c_id = modules.get_id("file:///c.js").unwrap();
      let d_id = modules.get_id("file:///d.js").unwrap();

      assert_eq!(
        modules.get_children(a_id),
        Some(&vec![
          "file:///b.js".to_string(),
          "file:///c.js".to_string()
        ])
      );
      assert_eq!(
        modules.get_children(b_id),
        Some(&vec!["file:///c.js".to_string()])
      );
      assert_eq!(
        modules.get_children(c_id),
        Some(&vec!["file:///d.js".to_string()])
      );
      assert_eq!(modules.get_children(d_id), Some(&vec![]));
    })
  }

  const CIRCULAR1_SRC: &str = r#"
    import "/circular2.js";
    Deno.core.print("circular1");
  "#;

  const CIRCULAR2_SRC: &str = r#"
    import "/circular3.js";
    Deno.core.print("circular2");
  "#;

  const CIRCULAR3_SRC: &str = r#"
    import "/circular1.js";
    import "/circular2.js";
    Deno.core.print("circular3");
  "#;

  #[test]
  fn test_circular_load() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let isolate = loader.isolate.clone();
      let isolate_ = isolate.clone();
      let modules = loader.modules.clone();
      let modules_ = modules.clone();
      let loads = loader.loads.clone();
      let recursive_load =
        RecursiveLoad::main("/circular1.js", None, loader, modules);
      let mut load_fut = recursive_load.get_future(isolate.clone()).boxed();
      let result = Pin::new(&mut load_fut).poll(&mut cx);
      assert!(result.is_ready());
      if let Poll::Ready(Ok(circular1_id)) = result {
        let mut isolate = isolate_.lock().unwrap();
        js_check(isolate.mod_evaluate(circular1_id));

        let l = loads.lock().unwrap();
        assert_eq!(
          l.to_vec(),
          vec![
            "file:///circular1.js",
            "file:///circular2.js",
            "file:///circular3.js"
          ]
        );

        let modules = modules_.lock().unwrap();

        assert_eq!(modules.get_id("file:///circular1.js"), Some(circular1_id));
        let circular2_id = modules.get_id("file:///circular2.js").unwrap();

        assert_eq!(
          modules.get_children(circular1_id),
          Some(&vec!["file:///circular2.js".to_string()])
        );

        assert_eq!(
          modules.get_children(circular2_id),
          Some(&vec!["file:///circular3.js".to_string()])
        );

        assert!(modules.get_id("file:///circular3.js").is_some());
        let circular3_id = modules.get_id("file:///circular3.js").unwrap();
        assert_eq!(
          modules.get_children(circular3_id),
          Some(&vec![
            "file:///circular1.js".to_string(),
            "file:///circular2.js".to_string()
          ])
        );
      } else {
        unreachable!();
      }
    })
  }

  const REDIRECT1_SRC: &str = r#"
    import "./redirect2.js";
    Deno.core.print("redirect1");
  "#;

  const REDIRECT2_SRC: &str = r#"
    import "./redirect3.js";
    Deno.core.print("redirect2");
  "#;

  const REDIRECT3_SRC: &str = r#"
    Deno.core.print("redirect3");
  "#;

  #[test]
  fn test_redirect_load() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let isolate = loader.isolate.clone();
      let isolate_ = isolate.clone();
      let modules = loader.modules.clone();
      let modules_ = modules.clone();
      let loads = loader.loads.clone();
      let recursive_load =
        RecursiveLoad::main("/redirect1.js", None, loader, modules);
      let mut load_fut = recursive_load.get_future(isolate.clone()).boxed();
      let result = Pin::new(&mut load_fut).poll(&mut cx);
      println!(">> result {:?}", result);
      assert!(result.is_ready());
      if let Poll::Ready(Ok(redirect1_id)) = result {
        let mut isolate = isolate_.lock().unwrap();
        js_check(isolate.mod_evaluate(redirect1_id));
        let l = loads.lock().unwrap();
        assert_eq!(
          l.to_vec(),
          vec![
            "file:///redirect1.js",
            "file:///redirect2.js",
            "file:///dir/redirect3.js"
          ]
        );

        let modules = modules_.lock().unwrap();

        assert_eq!(modules.get_id("file:///redirect1.js"), Some(redirect1_id));

        let redirect2_id = modules.get_id("file:///dir/redirect2.js").unwrap();
        assert!(modules.is_alias("file:///redirect2.js"));
        assert!(!modules.is_alias("file:///dir/redirect2.js"));
        assert_eq!(modules.get_id("file:///redirect2.js"), Some(redirect2_id));

        let redirect3_id = modules.get_id("file:///redirect3.js").unwrap();
        assert!(modules.is_alias("file:///dir/redirect3.js"));
        assert!(!modules.is_alias("file:///redirect3.js"));
        assert_eq!(
          modules.get_id("file:///dir/redirect3.js"),
          Some(redirect3_id)
        );
      } else {
        unreachable!();
      }
    })
  }

  // main.js
  const MAIN_SRC: &str = r#"
    // never_ready.js never loads.
    import "/never_ready.js";
    // slow.js resolves after one tick.
    import "/slow.js";
  "#;

  // slow.js
  const SLOW_SRC: &str = r#"
    // Circular import of never_ready.js
    // Does this trigger two Loader calls? It shouldn't.
    import "/never_ready.js";
    import "/a.js";
  "#;

  #[test]
  fn slow_never_ready_modules() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let isolate = loader.isolate.clone();
      let modules = loader.modules.clone();
      let loads = loader.loads.clone();
      let mut recursive_load =
        RecursiveLoad::main("/main.js", None, loader, modules)
          .get_future(isolate)
          .boxed();

      let result = recursive_load.poll_unpin(&mut cx);
      assert!(result.is_pending());

      // TODO(ry) Arguably the first time we poll only the following modules
      // should be loaded:
      //      "file:///main.js",
      //      "file:///never_ready.js",
      //      "file:///slow.js"
      // But due to current task notification in DelayedSourceCodeFuture they
      // all get loaded in a single poll. Also see the comment above
      // run_in_task.

      for _ in 0..10 {
        let result = recursive_load.poll_unpin(&mut cx);
        assert!(result.is_pending());
        let l = loads.lock().unwrap();
        assert_eq!(
          l.to_vec(),
          vec![
            "file:///main.js",
            "file:///never_ready.js",
            "file:///slow.js",
            "file:///a.js",
            "file:///b.js",
            "file:///c.js",
            "file:///d.js"
          ]
        );
      }
    })
  }

  // bad_import.js
  const BAD_IMPORT_SRC: &str = r#"
    import "foo";
  "#;

  #[test]
  fn loader_disappears_after_error() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let isolate = loader.isolate.clone();
      let modules = loader.modules.clone();
      let recursive_load =
        RecursiveLoad::main("/bad_import.js", None, loader, modules);
      let mut load_fut = recursive_load.get_future(isolate).boxed();
      let result = load_fut.poll_unpin(&mut cx);
      if let Poll::Ready(Err(err)) = result {
        assert_eq!(
          err.downcast_ref::<MockError>().unwrap(),
          &MockError::ResolveErr
        );
      } else {
        unreachable!();
      }
    })
  }

  const MAIN_WITH_CODE_SRC: &str = r#"
    import { b } from "/b.js";
    import { c } from "/c.js";
    if (b() != 'b') throw Error();
    if (c() != 'c') throw Error();
    if (!import.meta.main) throw Error();
    if (import.meta.url != 'file:///main_with_code.js') throw Error();
  "#;

  #[test]
  fn recursive_load_main_with_code() {
    run_in_task(|mut cx| {
      let loader = MockLoader::new();
      let modules = loader.modules.clone();
      let modules_ = modules.clone();
      let isolate = loader.isolate.clone();
      let isolate_ = isolate.clone();
      let loads = loader.loads.clone();
      // In default resolution code should be empty.
      // Instead we explicitly pass in our own code.
      // The behavior should be very similar to /a.js.
      let mut recursive_load = RecursiveLoad::main(
        "/main_with_code.js",
        Some(MAIN_WITH_CODE_SRC.to_owned()),
        loader,
        modules,
      );

      let main_id = loop {
        match recursive_load.poll_next_unpin(&mut cx) {
          Poll::Ready(Some(Ok(Event::Fetch(info)))) => {
            let mut isolate = isolate.lock().unwrap();
            recursive_load.register(info, &mut isolate).unwrap();
          }
          Poll::Ready(Some(Ok(Event::Instantiate(id)))) => break id,
          _ => panic!("unexpected result"),
        };
      };

      let mut isolate = isolate_.lock().unwrap();
      js_check(isolate.mod_evaluate(main_id));

      let l = loads.lock().unwrap();
      assert_eq!(
        l.to_vec(),
        vec!["file:///b.js", "file:///c.js", "file:///d.js"]
      );

      let modules = modules_.lock().unwrap();

      assert_eq!(modules.get_id("file:///main_with_code.js"), Some(main_id));
      let b_id = modules.get_id("file:///b.js").unwrap();
      let c_id = modules.get_id("file:///c.js").unwrap();
      let d_id = modules.get_id("file:///d.js").unwrap();

      assert_eq!(
        modules.get_children(main_id),
        Some(&vec![
          "file:///b.js".to_string(),
          "file:///c.js".to_string()
        ])
      );
      assert_eq!(
        modules.get_children(b_id),
        Some(&vec!["file:///c.js".to_string()])
      );
      assert_eq!(
        modules.get_children(c_id),
        Some(&vec!["file:///d.js".to_string()])
      );
      assert_eq!(modules.get_children(d_id), Some(&vec![]));
    })
  }

  #[test]
  fn empty_deps() {
    let modules = Modules::new();
    assert!(modules.deps("foo").is_none());
  }

  #[test]
  fn deps() {
    // "foo" -> "bar"
    let mut modules = Modules::new();
    modules.register(1, "foo");
    modules.register(2, "bar");
    modules.add_child(1, "bar");
    let maybe_deps = modules.deps("foo");
    assert!(maybe_deps.is_some());
    let mut foo_deps = maybe_deps.unwrap();
    assert_eq!(foo_deps.name, "foo");
    assert!(foo_deps.deps.is_some());
    let foo_children = foo_deps.deps.take().unwrap();
    assert_eq!(foo_children.len(), 1);
    let bar_deps = &foo_children[0];
    assert_eq!(bar_deps.name, "bar");
    assert_eq!(bar_deps.deps, Some(vec![]));
  }

  #[test]
  fn test_deps_to_json() {
    let mut modules = Modules::new();
    modules.register(1, "foo");
    modules.register(2, "bar");
    modules.register(3, "baz");
    modules.register(4, "zuh");
    modules.add_child(1, "bar");
    modules.add_child(1, "baz");
    modules.add_child(3, "zuh");
    let maybe_deps = modules.deps("foo");
    assert!(maybe_deps.is_some());
    assert_eq!(
      "[\"foo\",[[\"bar\",[]],[\"baz\",[[\"zuh\",[]]]]]]",
      maybe_deps.unwrap().to_json()
    );
  }
}
