// Copyright 2018-2025 the Deno authors. MIT license.

use crate::ModuleLoadResponse;
use crate::ModuleLoader;
use crate::ModuleSource;
use crate::ModuleSourceCode;
use crate::error::CoreError;
use crate::module_specifier::ModuleSpecifier;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleImportPhase;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoaderError;
use crate::modules::ModuleReference;
use crate::modules::ModuleRequest;
use crate::modules::RequestedModuleType;
use crate::modules::ResolutionKind;
use crate::modules::loaders::ModuleLoadReferrer;
use crate::modules::map::ModuleMap;
use crate::modules::module_map_data::ModuleSourceKind;
use crate::source_map::SourceMapApplication;
use crate::source_map::SourceMapper;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::Stream;
use futures::stream::TryStreamExt;
use std::cell::RefCell;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::Context;
use std::task::Poll;

use super::loaders::ModuleLoadOptions;

type ModuleLoadFuture = dyn Future<
  Output = Result<Option<(ModuleRequest, ModuleSource)>, ModuleLoaderError>,
>;

#[derive(Debug, Copy, Clone)]
pub enum SideModuleKind {
  Async,
  Sync,
}

/// Describes the entrypoint of a recursive module load.
#[derive(Debug)]
enum LoadInit {
  /// Main module specifier.
  Main(String),
  /// Module specifier for side module.
  Side {
    specifier: String,
    kind: SideModuleKind,
    code: RefCell<Option<String>>,
  },
  /// Dynamic import specifier with referrer and expected
  /// module type (which is determined by import assertion).
  DynamicImport(String, String, RequestedModuleType, ModuleImportPhase),
}

#[derive(Debug, Eq, PartialEq)]
enum LoadState {
  Init,
  LoadingRoot,
  LoadingImports,
  Done,
}

/// This future is used to implement parallel async module loading.
pub(crate) struct RecursiveModuleLoad {
  pub id: ModuleLoadId,
  pub root_module_id: Option<ModuleId>,
  init: LoadInit,
  state: LoadState,
  module_map_rc: Rc<ModuleMap>,
  pending: FuturesUnordered<Pin<Box<ModuleLoadFuture>>>,
  visited: HashSet<ModuleReference>,
  visited_as_alias: Rc<RefCell<HashSet<String>>>,
  pub root_module_reference: Option<ModuleReference>,
  // The loader is copied from `module_map_rc`, but its reference is cloned
  // ahead of time to avoid already-borrowed errors.
  loader: Rc<dyn ModuleLoader>,
}

impl Drop for RecursiveModuleLoad {
  fn drop(&mut self) {
    self.loader.finish_load();
  }
}

impl RecursiveModuleLoad {
  /// Starts a new asynchronous load of the module graph for given specifier.
  ///
  /// The module corresponding for the given `specifier` will be marked as
  // "the main module" (`import.meta.main` will return `true` for this module).
  pub(crate) fn main(specifier: String, module_map_rc: Rc<ModuleMap>) -> Self {
    Self::new(LoadInit::Main(specifier), module_map_rc)
  }

  /// Starts a new asynchronous load of the module graph for given specifier.
  pub(crate) fn side(
    specifier: String,
    module_map_rc: Rc<ModuleMap>,
    kind: SideModuleKind,
    code: Option<String>,
  ) -> Self {
    Self::new(
      LoadInit::Side {
        specifier,
        kind,
        code: RefCell::new(code),
      },
      module_map_rc,
    )
  }

  /// Starts a new asynchronous load of the module graph for given specifier
  /// that was imported using `import()`.
  pub(crate) fn dynamic_import(
    specifier: String,
    referrer: String,
    requested_module_type: RequestedModuleType,
    phase: ModuleImportPhase,
    module_map_rc: Rc<ModuleMap>,
  ) -> Self {
    Self::new(
      LoadInit::DynamicImport(
        specifier,
        referrer,
        requested_module_type,
        phase,
      ),
      module_map_rc,
    )
  }

  fn new(init: LoadInit, module_map_rc: Rc<ModuleMap>) -> Self {
    let id = module_map_rc.next_load_id();
    let loader = module_map_rc.loader.borrow().clone();
    let requested_module_type = match &init {
      LoadInit::DynamicImport(_, _, module_type, _) => module_type.clone(),
      _ => RequestedModuleType::None,
    };
    let mut load = Self {
      id,
      root_module_id: None,
      init,
      state: LoadState::Init,
      module_map_rc: module_map_rc.clone(),
      loader,
      pending: FuturesUnordered::new(),
      visited: HashSet::new(),
      visited_as_alias: Default::default(),
      root_module_reference: None,
    };
    // FIXME(bartlomieju): this seems fishy
    // Ignore the error here, let it be hit in `Stream::poll_next()`.
    if let Ok(root_specifier) = load.resolve_root()
      && let Some(module_id) =
        module_map_rc.get_id(root_specifier.as_str(), requested_module_type)
    {
      load.root_module_id = Some(module_id);
    }
    load
  }

  pub fn resolve_root(&self) -> Result<ModuleSpecifier, CoreError> {
    match self.init {
      LoadInit::Main(ref specifier) => {
        self
          .module_map_rc
          .resolve(specifier, ".", ResolutionKind::MainModule)
      }
      LoadInit::Side { ref specifier, .. } => {
        self
          .module_map_rc
          .resolve(specifier, ".", ResolutionKind::Import)
      }
      LoadInit::DynamicImport(ref specifier, ref referrer, _, _) => self
        .module_map_rc
        .resolve(specifier, referrer, ResolutionKind::DynamicImport),
    }
  }

  pub(crate) async fn prepare(&self) -> Result<(), CoreError> {
    let (
      module_specifier,
      maybe_referrer,
      maybe_code,
      requested_module_type,
      is_synchronous,
    ) = match &self.init {
      LoadInit::Main(specifier) => {
        let spec = self.module_map_rc.resolve(
          specifier,
          ".",
          ResolutionKind::MainModule,
        )?;
        (spec, None, None, RequestedModuleType::None, false)
      }
      LoadInit::Side {
        specifier,
        kind,
        code,
      } => {
        let spec =
          self
            .module_map_rc
            .resolve(specifier, ".", ResolutionKind::Import)?;
        (
          spec,
          None,
          code.take(),
          RequestedModuleType::None,
          match kind {
            SideModuleKind::Async => false,
            SideModuleKind::Sync => true,
          },
        )
      }
      LoadInit::DynamicImport(
        specifier,
        referrer,
        requested_module_type,
        _,
      ) => {
        let spec = self.module_map_rc.resolve(
          specifier,
          referrer,
          ResolutionKind::DynamicImport,
        )?;
        (
          spec,
          Some(referrer.to_string()),
          None,
          requested_module_type.clone(),
          false,
        )
      }
    };

    self
      .loader
      .prepare_load(
        &module_specifier,
        maybe_referrer,
        maybe_code,
        ModuleLoadOptions {
          is_synchronous,
          is_dynamic_import: self.is_dynamic_import(),
          requested_module_type,
        },
      )
      .await
      .map_err(|e| e.into())
  }

  fn is_currently_loading_main_module(&self) -> bool {
    !self.is_dynamic_import()
      && matches!(self.init, LoadInit::Main(..))
      && self.state == LoadState::LoadingRoot
  }

  fn is_dynamic_import(&self) -> bool {
    matches!(self.init, LoadInit::DynamicImport(..))
  }

  fn is_synchronous(&self) -> bool {
    matches!(
      self.init,
      LoadInit::Side {
        kind: SideModuleKind::Sync,
        ..
      }
    )
  }

  pub(crate) fn register_and_recurse(
    &mut self,
    scope: &mut v8::PinScope,
    module_request: &ModuleRequest,
    mut module_source: ModuleSource,
  ) -> Result<(), ModuleError> {
    if let Some(source_kind) =
      ModuleSourceKind::from_module_type(&module_source.module_type)
    {
      match source_kind {
        ModuleSourceKind::Wasm => {
          module_source = self.module_map_rc.new_wasm_module_source(
            scope,
            &module_request.reference,
            module_source,
          )?;
        }
      }
      if module_request.phase == ModuleImportPhase::Source {
        if self.pending.is_empty() {
          self.state = LoadState::Done;
        }
        return Ok(());
      }
    } else if module_request.phase == ModuleImportPhase::Source {
      let message = v8::String::new(
        scope,
        &format!(
          "Source phase imports are not supported for {} modules",
          &module_source.module_type
        ),
      )
      .unwrap();
      let exception = v8::Exception::reference_error(scope, message);
      return Err(ModuleError::Exception(v8::Global::new(scope, exception)));
    }

    let code = module_source.cheap_copy_code();

    let module_id = self.module_map_rc.new_module(
      scope,
      self.is_currently_loading_main_module(),
      self.is_dynamic_import(),
      module_source,
    )?;

    self.register_and_recurse_inner(
      module_id,
      &module_request.reference,
      Some(&code),
    );

    if self.state == LoadState::LoadingRoot {
      self.root_module_id = Some(module_id);
      self.state = LoadState::LoadingImports;
    }
    if self.pending.is_empty() {
      self.state = LoadState::Done;
    }

    Ok(())
  }

  fn register_and_recurse_inner(
    &mut self,
    module_id: usize,
    module_reference: &ModuleReference,
    code: Option<&ModuleSourceCode>,
  ) {
    // Recurse the module's imports. There are two cases for each import:
    // 1. If the module is not in the module map, start a new load for it in
    //    `self.pending`. The result of that load should eventually be passed to
    //    this function for recursion.
    // 2. If the module is already in the module map, queue it up to be
    //    recursed synchronously here.
    // This robustly ensures that the whole graph is in the module map before
    // `LoadState::Done` is set.
    let mut already_registered = VecDeque::new();
    already_registered.push_back((module_id, module_reference.clone()));
    self.visited.insert(module_reference.clone());
    while let Some((module_id, module_reference)) =
      already_registered.pop_front()
    {
      let referrer = &module_reference.specifier;
      let imports = self
        .module_map_rc
        .get_requested_modules(module_id)
        .unwrap()
        .clone();
      for module_request in imports {
        if !self.visited.contains(&module_request.reference)
          && !self
            .visited_as_alias
            .borrow()
            .contains(module_request.reference.specifier.as_str())
        {
          match self.module_map_rc.get_id(
            module_request.reference.specifier.as_str(),
            &module_request.reference.requested_module_type,
          ) {
            Some(module_id) => {
              already_registered
                .push_back((module_id, module_request.reference.clone()));
            }
            _ => {
              let request = module_request.clone();
              let visited_as_alias = self.visited_as_alias.clone();
              let referrer = code.and_then(|code| {
                let source_offset = request.referrer_source_offset?;
                source_mapped_module_load_referrer(
                  &self.module_map_rc.source_mapper,
                  referrer,
                  code,
                  source_offset,
                )
              });
              let loader = self.loader.clone();
              let is_dynamic_import = self.is_dynamic_import();
              let is_synchronous = self.is_synchronous();
              let requested_module_type =
                request.reference.requested_module_type.clone();
              let fut = async move {
                // `visited_as_alias` unlike `visited` is checked as late as
                // possible because it can only be populated after completed
                // loads, meaning a duplicate load future may have already been
                // dispatched before we know it's a duplicate.
                if visited_as_alias
                  .borrow()
                  .contains(request.reference.specifier.as_str())
                {
                  return Ok(None);
                }

                let load_response = loader.load(
                  &request.reference.specifier,
                  referrer.as_ref(),
                  ModuleLoadOptions {
                    is_dynamic_import,
                    is_synchronous,
                    requested_module_type,
                  },
                );

                let load_result = match load_response {
                  ModuleLoadResponse::Sync(result) => result,
                  ModuleLoadResponse::Async(fut) => fut.await,
                };
                if let Ok(source) = &load_result
                  && let Some(found_specifier) = &source.module_url_found
                {
                  visited_as_alias
                    .borrow_mut()
                    .insert(found_specifier.as_str().to_string());
                }
                load_result.map(|s| Some((request, s)))
              };
              self.pending.push(fut.boxed_local());
            }
          }
          self.visited.insert(module_request.reference);
        }
      }
    }
  }
}

impl Stream for RecursiveModuleLoad {
  type Item = Result<(ModuleRequest, ModuleSource), CoreError>;

  fn poll_next(
    self: Pin<&mut Self>,
    cx: &mut Context,
  ) -> Poll<Option<Self::Item>> {
    let inner = self.get_mut();
    // IMPORTANT: Do not borrow `inner.module_map_rc` here. It may not be
    // available.
    match inner.state {
      LoadState::Init => {
        let module_specifier = match inner.resolve_root() {
          Ok(url) => url,
          Err(error) => {
            return Poll::Ready(Some(Err(error)));
          }
        };
        let (requested_module_type, phase) = match &inner.init {
          LoadInit::DynamicImport(_, _, module_type, phase) => {
            (module_type.clone(), *phase)
          }
          _ => (RequestedModuleType::None, ModuleImportPhase::Evaluation),
        };
        let module_request = ModuleRequest {
          reference: ModuleReference {
            specifier: module_specifier.clone(),
            requested_module_type: requested_module_type.clone(),
          },
          specifier_key: None,
          referrer_source_offset: None,
          phase,
        };
        inner.root_module_reference = Some(module_request.reference.clone());
        let load_fut = if phase == ModuleImportPhase::Evaluation
          && let Some(module_id) = inner.root_module_id
        {
          // If the inner future is already in the map, we might be done (assuming there are no pending
          // loads).
          inner.register_and_recurse_inner(
            module_id,
            &module_request.reference,
            None,
          );
          if inner.pending.is_empty() {
            inner.state = LoadState::Done;
          } else {
            inner.state = LoadState::LoadingImports;
          }
          // Internally re-poll using the new state to avoid spinning the event loop again.
          return Self::poll_next(Pin::new(inner), cx);
        } else {
          let loader = inner.loader.clone();
          let is_dynamic_import = inner.is_dynamic_import();
          let is_synchronous = inner.is_synchronous();
          let requested_module_type = requested_module_type.clone();
          async move {
            let load_response = loader.load(
              &module_specifier,
              None,
              ModuleLoadOptions {
                is_dynamic_import,
                is_synchronous,
                requested_module_type,
              },
            );
            let result = match load_response {
              ModuleLoadResponse::Sync(result) => result,
              ModuleLoadResponse::Async(fut) => fut.await,
            };
            result.map(|s| Some((module_request, s)))
          }
          .boxed_local()
        };
        inner.pending.push(load_fut);
        inner.state = LoadState::LoadingRoot;
        inner.try_poll_next_unpin(cx)
      }
      LoadState::LoadingRoot | LoadState::LoadingImports => {
        // Poll the futures that load the source code of the modules
        match inner.pending.try_poll_next_unpin(cx)? {
          Poll::Ready(None) => unreachable!(),
          Poll::Ready(Some(None)) => {
            // The future resolves to None when loading an already visited redirect
            if inner.pending.is_empty() {
              inner.state = LoadState::Done;
              Poll::Ready(None)
            } else {
              // Force re-poll to make sure new ModuleLoadFuture's wakers are registered
              inner.try_poll_next_unpin(cx)
            }
          }
          Poll::Ready(Some(Some(info))) => Poll::Ready(Some(Ok(info))),
          Poll::Pending => Poll::Pending,
        }
      }
      LoadState::Done => Poll::Ready(None),
    }
  }
}

fn source_mapped_module_load_referrer(
  source_mapper: &RefCell<SourceMapper>,
  referrer: &ModuleSpecifier,
  code: &ModuleSourceCode,
  source_offset: i32,
) -> Option<ModuleLoadReferrer> {
  // 1-based.
  let (line_number, column_number) = code
    .as_bytes()
    .split_at_checked(source_offset as usize)?
    .0
    .iter()
    .enumerate()
    .filter(|(_, c)| **c as char == '\n')
    .enumerate()
    .last()
    .map(|(n, (i, _))| (n as u32 + 2, source_offset as u32 - i as u32))
    .unwrap_or_else(|| (1, source_offset as u32 + 1));
  let (specifier, line_number, column_number) = match source_mapper
    .borrow_mut()
    .apply_source_map(referrer.as_str(), line_number, column_number)
  {
    SourceMapApplication::Unchanged => {
      (referrer.clone(), line_number as _, column_number as _)
    }
    SourceMapApplication::LineAndColumn {
      line_number,
      column_number,
    } => (referrer.clone(), line_number as _, column_number as _),
    SourceMapApplication::LineAndColumnAndFileName {
      file_name,
      line_number,
      column_number,
    } => (
      ModuleSpecifier::parse(&file_name).ok()?,
      line_number as _,
      column_number as _,
    ),
  };
  Some(ModuleLoadReferrer {
    specifier,
    line_number,
    column_number,
  })
}
