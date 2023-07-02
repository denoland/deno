// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use crate::error::exception_to_err_result;
use crate::error::generic_error;
use crate::error::throw_type_error;
use crate::fast_string::FastString;
use crate::modules::get_asserted_module_type_from_assertions;
use crate::modules::parse_import_assertions;
use crate::modules::validate_import_assertions;
use crate::modules::ImportAssertionsKind;
use crate::modules::ModuleCode;
use crate::modules::ModuleError;
use crate::modules::ModuleId;
use crate::modules::ModuleInfo;
use crate::modules::ModuleLoadId;
use crate::modules::ModuleLoader;
use crate::modules::ModuleName;
use crate::modules::ModuleRequest;
use crate::modules::ModuleType;
use crate::modules::NoopModuleLoader;
use crate::modules::PrepareLoadFuture;
use crate::modules::RecursiveModuleLoad;
use crate::modules::ResolutionKind;
use crate::runtime::JsRuntime;
use crate::runtime::SnapshottedData;
use anyhow::Error;
use futures::future::FutureExt;
use futures::stream::FuturesUnordered;
use futures::stream::StreamFuture;
use std::cell::RefCell;
use std::collections::HashMap;
use std::pin::Pin;
use std::rc::Rc;

use super::AssertedModuleType;

pub const BOM_CHAR: &[u8] = &[0xef, 0xbb, 0xbf];

/// Strips the byte order mark from the provided text if it exists.
fn strip_bom(source_code: &[u8]) -> &[u8] {
  if source_code.starts_with(BOM_CHAR) {
    &source_code[BOM_CHAR.len()..]
  } else {
    source_code
  }
}

/// A symbolic module entity.
#[derive(Debug, PartialEq)]
pub(crate) enum SymbolicModule {
  /// This module is an alias to another module.
  /// This is useful such that multiple names could point to
  /// the same underlying module (particularly due to redirects).
  Alias(ModuleName),
  /// This module associates with a V8 module by id.
  Mod(ModuleId),
}

/// A collection of JS modules.
pub(crate) struct ModuleMap {
  // Handling of specifiers and v8 objects
  pub handles: Vec<v8::Global<v8::Module>>,
  pub info: Vec<ModuleInfo>,
  pub(crate) by_name_js: HashMap<ModuleName, SymbolicModule>,
  pub(crate) by_name_json: HashMap<ModuleName, SymbolicModule>,
  pub(crate) next_load_id: ModuleLoadId,

  // Handling of futures for loading module sources
  pub loader: Rc<dyn ModuleLoader>,
  pub(crate) dynamic_import_map:
    HashMap<ModuleLoadId, v8::Global<v8::PromiseResolver>>,
  pub(crate) preparing_dynamic_imports:
    FuturesUnordered<Pin<Box<PrepareLoadFuture>>>,
  pub(crate) pending_dynamic_imports:
    FuturesUnordered<StreamFuture<RecursiveModuleLoad>>,

  // This store is used temporarily, to forward parsed JSON
  // value from `new_json_module` to `json_module_evaluation_steps`
  json_value_store: HashMap<v8::Global<v8::Module>, v8::Global<v8::Value>>,
}

impl ModuleMap {
  pub fn collect_modules(
    &self,
  ) -> Vec<(AssertedModuleType, &ModuleName, &SymbolicModule)> {
    let mut output = vec![];
    for module_type in [
      AssertedModuleType::JavaScriptOrWasm,
      AssertedModuleType::Json,
    ] {
      output.extend(
        self
          .by_name(module_type)
          .iter()
          .map(|x| (module_type, x.0, x.1)),
      )
    }
    output
  }

  #[cfg(debug_assertions)]
  pub(crate) fn assert_all_modules_evaluated(
    &self,
    scope: &mut v8::HandleScope,
  ) {
    let mut not_evaluated = vec![];

    for (i, handle) in self.handles.iter().enumerate() {
      let module = v8::Local::new(scope, handle);
      if !matches!(module.get_status(), v8::ModuleStatus::Evaluated) {
        not_evaluated.push(self.info[i].name.as_str().to_string());
      }
    }

    if !not_evaluated.is_empty() {
      let mut msg = "Following modules were not evaluated; make sure they are imported from other code:\n".to_string();
      for m in not_evaluated {
        msg.push_str(&format!("  - {}\n", m));
      }
      panic!("{}", msg);
    }
  }

  pub fn serialize_for_snapshotting(
    &self,
    scope: &mut v8::HandleScope,
  ) -> SnapshottedData {
    let array = v8::Array::new(scope, 3);

    let next_load_id = v8::Integer::new(scope, self.next_load_id);
    array.set_index(scope, 0, next_load_id.into());

    let info_arr = v8::Array::new(scope, self.info.len() as i32);
    for (i, info) in self.info.iter().enumerate() {
      let module_info_arr = v8::Array::new(scope, 5);

      let id = v8::Integer::new(scope, info.id as i32);
      module_info_arr.set_index(scope, 0, id.into());

      let main = v8::Boolean::new(scope, info.main);
      module_info_arr.set_index(scope, 1, main.into());

      let name = info.name.v8(scope);
      module_info_arr.set_index(scope, 2, name.into());

      let array_len = 2 * info.requests.len() as i32;
      let requests_arr = v8::Array::new(scope, array_len);
      for (i, request) in info.requests.iter().enumerate() {
        let specifier = v8::String::new_from_one_byte(
          scope,
          request.specifier.as_bytes(),
          v8::NewStringType::Normal,
        )
        .unwrap();
        requests_arr.set_index(scope, 2 * i as u32, specifier.into());

        let asserted_module_type =
          v8::Integer::new(scope, request.asserted_module_type as i32);
        requests_arr.set_index(
          scope,
          (2 * i) as u32 + 1,
          asserted_module_type.into(),
        );
      }
      module_info_arr.set_index(scope, 3, requests_arr.into());

      let module_type = v8::Integer::new(scope, info.module_type as i32);
      module_info_arr.set_index(scope, 4, module_type.into());

      info_arr.set_index(scope, i as u32, module_info_arr.into());
    }
    array.set_index(scope, 1, info_arr.into());

    let by_name = self.collect_modules();
    let by_name_array = v8::Array::new(scope, by_name.len() as i32);
    {
      for (i, (module_type, name, module)) in by_name.into_iter().enumerate() {
        let arr = v8::Array::new(scope, 3);

        let specifier = name.v8(scope);
        arr.set_index(scope, 0, specifier.into());

        let asserted_module_type = v8::Integer::new(scope, module_type as i32);
        arr.set_index(scope, 1, asserted_module_type.into());

        let symbolic_module: v8::Local<v8::Value> = match module {
          SymbolicModule::Alias(alias) => {
            let alias = v8::String::new_from_one_byte(
              scope,
              alias.as_bytes(),
              v8::NewStringType::Normal,
            )
            .unwrap();
            alias.into()
          }
          SymbolicModule::Mod(id) => {
            let id = v8::Integer::new(scope, *id as i32);
            id.into()
          }
        };
        arr.set_index(scope, 2, symbolic_module);

        by_name_array.set_index(scope, i as u32, arr.into());
      }
    }
    array.set_index(scope, 2, by_name_array.into());

    let array_global = v8::Global::new(scope, array);

    let handles = self.handles.clone();
    SnapshottedData {
      module_map_data: array_global,
      module_handles: handles,
    }
  }

  pub fn update_with_snapshotted_data(
    &mut self,
    scope: &mut v8::HandleScope,
    snapshotted_data: SnapshottedData,
  ) {
    let local_data: v8::Local<v8::Array> =
      v8::Local::new(scope, snapshotted_data.module_map_data);

    {
      let next_load_id = local_data.get_index(scope, 0).unwrap();
      assert!(next_load_id.is_int32());
      let integer = next_load_id.to_integer(scope).unwrap();
      let val = integer.int32_value(scope).unwrap();
      self.next_load_id = val;
    }

    {
      let info_val = local_data.get_index(scope, 1).unwrap();

      let info_arr: v8::Local<v8::Array> = info_val.try_into().unwrap();
      let len = info_arr.length() as usize;
      // Over allocate so executing a few scripts doesn't have to resize this vec.
      let mut info = Vec::with_capacity(len + 16);

      for i in 0..len {
        let module_info_arr: v8::Local<v8::Array> = info_arr
          .get_index(scope, i as u32)
          .unwrap()
          .try_into()
          .unwrap();
        let id = module_info_arr
          .get_index(scope, 0)
          .unwrap()
          .to_integer(scope)
          .unwrap()
          .value() as ModuleId;

        let main = module_info_arr
          .get_index(scope, 1)
          .unwrap()
          .to_boolean(scope)
          .is_true();

        let name = module_info_arr
          .get_index(scope, 2)
          .unwrap()
          .to_rust_string_lossy(scope)
          .into();

        let requests_arr: v8::Local<v8::Array> = module_info_arr
          .get_index(scope, 3)
          .unwrap()
          .try_into()
          .unwrap();
        let len = (requests_arr.length() as usize) / 2;
        let mut requests = Vec::with_capacity(len);
        for i in 0..len {
          let specifier = requests_arr
            .get_index(scope, (2 * i) as u32)
            .unwrap()
            .to_rust_string_lossy(scope);
          let asserted_module_type_no = requests_arr
            .get_index(scope, (2 * i + 1) as u32)
            .unwrap()
            .to_integer(scope)
            .unwrap()
            .value();
          let asserted_module_type = match asserted_module_type_no {
            0 => AssertedModuleType::JavaScriptOrWasm,
            1 => AssertedModuleType::Json,
            _ => unreachable!(),
          };
          requests.push(ModuleRequest {
            specifier,
            asserted_module_type,
          });
        }

        let module_type_no = module_info_arr
          .get_index(scope, 4)
          .unwrap()
          .to_integer(scope)
          .unwrap()
          .value();
        let module_type = match module_type_no {
          0 => ModuleType::JavaScript,
          1 => ModuleType::Json,
          _ => unreachable!(),
        };

        let module_info = ModuleInfo {
          id,
          main,
          name,
          requests,
          module_type,
        };
        info.push(module_info);
      }

      self.info = info;
    }

    self
      .by_name_mut(AssertedModuleType::JavaScriptOrWasm)
      .clear();
    self.by_name_mut(AssertedModuleType::Json).clear();

    {
      let by_name_arr: v8::Local<v8::Array> =
        local_data.get_index(scope, 2).unwrap().try_into().unwrap();
      let len = by_name_arr.length() as usize;

      for i in 0..len {
        let arr: v8::Local<v8::Array> = by_name_arr
          .get_index(scope, i as u32)
          .unwrap()
          .try_into()
          .unwrap();

        let specifier =
          arr.get_index(scope, 0).unwrap().to_rust_string_lossy(scope);
        let asserted_module_type = match arr
          .get_index(scope, 1)
          .unwrap()
          .to_integer(scope)
          .unwrap()
          .value()
        {
          0 => AssertedModuleType::JavaScriptOrWasm,
          1 => AssertedModuleType::Json,
          _ => unreachable!(),
        };

        let symbolic_module_val = arr.get_index(scope, 2).unwrap();
        let val = if symbolic_module_val.is_number() {
          SymbolicModule::Mod(
            symbolic_module_val
              .to_integer(scope)
              .unwrap()
              .value()
              .try_into()
              .unwrap(),
          )
        } else {
          SymbolicModule::Alias(
            symbolic_module_val.to_rust_string_lossy(scope).into(),
          )
        };

        self
          .by_name_mut(asserted_module_type)
          .insert(specifier.into(), val);
      }
    }

    self.handles = snapshotted_data.module_handles;
  }

  pub(crate) fn new(loader: Rc<dyn ModuleLoader>) -> ModuleMap {
    Self {
      handles: vec![],
      info: vec![],
      by_name_js: HashMap::new(),
      by_name_json: HashMap::new(),
      next_load_id: 1,
      loader,
      dynamic_import_map: HashMap::new(),
      preparing_dynamic_imports: FuturesUnordered::new(),
      pending_dynamic_imports: FuturesUnordered::new(),
      json_value_store: HashMap::new(),
    }
  }

  /// Get module id, following all aliases in case of module specifier
  /// that had been redirected.
  pub(crate) fn get_id(
    &self,
    name: impl AsRef<str>,
    asserted_module_type: AssertedModuleType,
  ) -> Option<ModuleId> {
    let map = self.by_name(asserted_module_type);
    let first_symbolic_module = map.get(name.as_ref())?;
    let mut mod_name = match first_symbolic_module {
      SymbolicModule::Mod(mod_id) => return Some(*mod_id),
      SymbolicModule::Alias(target) => target,
    };
    loop {
      let symbolic_module = map.get(mod_name.as_ref())?;
      match symbolic_module {
        SymbolicModule::Alias(target) => {
          debug_assert!(mod_name != target);
          mod_name = target;
        }
        SymbolicModule::Mod(mod_id) => return Some(*mod_id),
      }
    }
  }

  pub(crate) fn new_json_module(
    &mut self,
    scope: &mut v8::HandleScope,
    name: ModuleName,
    source: ModuleCode,
  ) -> Result<ModuleId, ModuleError> {
    let name_str = name.v8(scope);
    let source_str = v8::String::new_from_utf8(
      scope,
      strip_bom(source.as_bytes()),
      v8::NewStringType::Normal,
    )
    .unwrap();

    let tc_scope = &mut v8::TryCatch::new(scope);

    let parsed_json = match v8::json::parse(tc_scope, source_str) {
      Some(parsed_json) => parsed_json,
      None => {
        assert!(tc_scope.has_caught());
        let exception = tc_scope.exception().unwrap();
        let exception = v8::Global::new(tc_scope, exception);
        return Err(ModuleError::Exception(exception));
      }
    };

    let export_names = [v8::String::new(tc_scope, "default").unwrap()];
    let module = v8::Module::create_synthetic_module(
      tc_scope,
      name_str,
      &export_names,
      json_module_evaluation_steps,
    );

    let handle = v8::Global::<v8::Module>::new(tc_scope, module);
    let value_handle = v8::Global::<v8::Value>::new(tc_scope, parsed_json);
    self.json_value_store.insert(handle.clone(), value_handle);

    let id =
      self.create_module_info(name, ModuleType::Json, handle, false, vec![]);

    Ok(id)
  }

  /// Create and compile an ES module.
  pub(crate) fn new_es_module(
    &mut self,
    scope: &mut v8::HandleScope,
    main: bool,
    name: ModuleName,
    source: ModuleCode,
    is_dynamic_import: bool,
  ) -> Result<ModuleId, ModuleError> {
    let name_str = name.v8(scope);
    let source_str = source.v8(scope);

    let origin = module_origin(scope, name_str);
    let source = v8::script_compiler::Source::new(source_str, Some(&origin));

    let tc_scope = &mut v8::TryCatch::new(scope);

    let maybe_module = v8::script_compiler::compile_module(tc_scope, source);

    if tc_scope.has_caught() {
      assert!(maybe_module.is_none());
      let exception = tc_scope.exception().unwrap();
      let exception = v8::Global::new(tc_scope, exception);
      return Err(ModuleError::Exception(exception));
    }

    let module = maybe_module.unwrap();

    let mut requests: Vec<ModuleRequest> = vec![];
    let module_requests = module.get_module_requests();
    for i in 0..module_requests.length() {
      let module_request = v8::Local::<v8::ModuleRequest>::try_from(
        module_requests.get(tc_scope, i).unwrap(),
      )
      .unwrap();
      let import_specifier = module_request
        .get_specifier()
        .to_rust_string_lossy(tc_scope);

      let import_assertions = module_request.get_import_assertions();

      let assertions = parse_import_assertions(
        tc_scope,
        import_assertions,
        ImportAssertionsKind::StaticImport,
      );

      // FIXME(bartomieju): there are no stack frames if exception
      // is thrown here
      validate_import_assertions(tc_scope, &assertions);
      if tc_scope.has_caught() {
        let exception = tc_scope.exception().unwrap();
        let exception = v8::Global::new(tc_scope, exception);
        return Err(ModuleError::Exception(exception));
      }

      let module_specifier = match self.loader.resolve(
        &import_specifier,
        name.as_ref(),
        if is_dynamic_import {
          ResolutionKind::DynamicImport
        } else {
          ResolutionKind::Import
        },
      ) {
        Ok(s) => s,
        Err(e) => return Err(ModuleError::Other(e)),
      };
      let asserted_module_type =
        get_asserted_module_type_from_assertions(&assertions);
      let request = ModuleRequest {
        specifier: module_specifier.to_string(),
        asserted_module_type,
      };
      requests.push(request);
    }

    if main {
      let maybe_main_module = self.info.iter().find(|module| module.main);
      if let Some(main_module) = maybe_main_module {
        return Err(ModuleError::Other(generic_error(
          format!("Trying to create \"main\" module ({:?}), when one already exists ({:?})",
          name.as_ref(),
          main_module.name,
        ))));
      }
    }

    let handle = v8::Global::<v8::Module>::new(tc_scope, module);
    let id = self.create_module_info(
      name,
      ModuleType::JavaScript,
      handle,
      main,
      requests,
    );

    Ok(id)
  }

  pub(crate) fn instantiate_module(
    &mut self,
    scope: &mut v8::HandleScope,
    id: ModuleId,
  ) -> Result<(), v8::Global<v8::Value>> {
    let tc_scope = &mut v8::TryCatch::new(scope);

    let module = self
      .get_handle(id)
      .map(|handle| v8::Local::new(tc_scope, handle))
      .expect("ModuleInfo not found");

    if module.get_status() == v8::ModuleStatus::Errored {
      return Err(v8::Global::new(tc_scope, module.get_exception()));
    }

    tc_scope.set_slot(self as *const _);
    let instantiate_result =
      module.instantiate_module(tc_scope, Self::module_resolve_callback);
    tc_scope.remove_slot::<*const Self>();
    if instantiate_result.is_none() {
      let exception = tc_scope.exception().unwrap();
      return Err(v8::Global::new(tc_scope, exception));
    }

    Ok(())
  }

  /// Called by V8 during `JsRuntime::instantiate_module`. This is only used internally, so we use the Isolate's annex
  /// to propagate a &Self.
  fn module_resolve_callback<'s>(
    context: v8::Local<'s, v8::Context>,
    specifier: v8::Local<'s, v8::String>,
    import_assertions: v8::Local<'s, v8::FixedArray>,
    referrer: v8::Local<'s, v8::Module>,
  ) -> Option<v8::Local<'s, v8::Module>> {
    // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
    let scope = &mut unsafe { v8::CallbackScope::new(context) };

    let module_map =
      // SAFETY: We retrieve the pointer from the slot, having just set it a few stack frames up
      unsafe { scope.get_slot::<*const Self>().unwrap().as_ref().unwrap() };

    let referrer_global = v8::Global::new(scope, referrer);

    let referrer_info = module_map
      .get_info(&referrer_global)
      .expect("ModuleInfo not found");
    let referrer_name = referrer_info.name.as_str();

    let specifier_str = specifier.to_rust_string_lossy(scope);

    let assertions = parse_import_assertions(
      scope,
      import_assertions,
      ImportAssertionsKind::StaticImport,
    );
    let maybe_module = module_map.resolve_callback(
      scope,
      &specifier_str,
      referrer_name,
      assertions,
    );
    if let Some(module) = maybe_module {
      return Some(module);
    }

    let msg = format!(
      r#"Cannot resolve module "{specifier_str}" from "{referrer_name}""#
    );
    throw_type_error(scope, msg);
    None
  }

  /// Called by `module_resolve_callback` during module instantiation.
  fn resolve_callback<'s>(
    &self,
    scope: &mut v8::HandleScope<'s>,
    specifier: &str,
    referrer: &str,
    import_assertions: HashMap<String, String>,
  ) -> Option<v8::Local<'s, v8::Module>> {
    let resolved_specifier = self
      .loader
      .resolve(specifier, referrer, ResolutionKind::Import)
      .expect("Module should have been already resolved");

    let module_type =
      get_asserted_module_type_from_assertions(&import_assertions);

    if let Some(id) = self.get_id(resolved_specifier.as_str(), module_type) {
      if let Some(handle) = self.get_handle(id) {
        return Some(v8::Local::new(scope, handle));
      }
    }

    None
  }

  pub(crate) fn clear(&mut self) {
    *self = Self::new(self.loader.clone())
  }

  pub(crate) fn get_handle_by_name(
    &self,
    name: impl AsRef<str>,
  ) -> Option<v8::Global<v8::Module>> {
    let id = self
      .get_id(name.as_ref(), AssertedModuleType::JavaScriptOrWasm)
      .or_else(|| self.get_id(name.as_ref(), AssertedModuleType::Json))?;
    self.get_handle(id)
  }

  pub(crate) fn inject_handle(
    &mut self,
    name: ModuleName,
    module_type: ModuleType,
    handle: v8::Global<v8::Module>,
  ) {
    self.create_module_info(name, module_type, handle, false, vec![]);
  }

  fn create_module_info(
    &mut self,
    name: FastString,
    module_type: ModuleType,
    handle: v8::Global<v8::Module>,
    main: bool,
    requests: Vec<ModuleRequest>,
  ) -> ModuleId {
    let id = self.handles.len();
    let (name1, name2) = name.into_cheap_copy();
    self
      .by_name_mut(module_type.into())
      .insert(name1, SymbolicModule::Mod(id));
    self.handles.push(handle);
    self.info.push(ModuleInfo {
      id,
      main,
      name: name2,
      requests,
      module_type,
    });

    id
  }

  pub(crate) fn get_requested_modules(
    &self,
    id: ModuleId,
  ) -> Option<&Vec<ModuleRequest>> {
    self.info.get(id).map(|i| &i.requests)
  }

  fn is_registered(
    &self,
    specifier: impl AsRef<str>,
    asserted_module_type: AssertedModuleType,
  ) -> bool {
    if let Some(id) = self.get_id(specifier.as_ref(), asserted_module_type) {
      let info = self.get_info_by_id(id).unwrap();
      return asserted_module_type == info.module_type.into();
    }

    false
  }

  pub(crate) fn by_name(
    &self,
    asserted_module_type: AssertedModuleType,
  ) -> &HashMap<ModuleName, SymbolicModule> {
    match asserted_module_type {
      AssertedModuleType::Json => &self.by_name_json,
      AssertedModuleType::JavaScriptOrWasm => &self.by_name_js,
    }
  }

  pub(crate) fn by_name_mut(
    &mut self,
    asserted_module_type: AssertedModuleType,
  ) -> &mut HashMap<ModuleName, SymbolicModule> {
    match asserted_module_type {
      AssertedModuleType::Json => &mut self.by_name_json,
      AssertedModuleType::JavaScriptOrWasm => &mut self.by_name_js,
    }
  }

  pub(crate) fn alias(
    &mut self,
    name: FastString,
    asserted_module_type: AssertedModuleType,
    target: FastString,
  ) {
    debug_assert_ne!(name, target);
    self
      .by_name_mut(asserted_module_type)
      .insert(name, SymbolicModule::Alias(target));
  }

  #[cfg(test)]
  pub(crate) fn is_alias(
    &self,
    name: &str,
    asserted_module_type: AssertedModuleType,
  ) -> bool {
    let cond = self.by_name(asserted_module_type).get(name);
    matches!(cond, Some(SymbolicModule::Alias(_)))
  }

  pub(crate) fn get_handle(
    &self,
    id: ModuleId,
  ) -> Option<v8::Global<v8::Module>> {
    self.handles.get(id).cloned()
  }

  pub(crate) fn get_info(
    &self,
    global: &v8::Global<v8::Module>,
  ) -> Option<&ModuleInfo> {
    if let Some(id) = self.handles.iter().position(|module| module == global) {
      return self.info.get(id);
    }

    None
  }

  pub(crate) fn get_info_by_id(&self, id: ModuleId) -> Option<&ModuleInfo> {
    self.info.get(id)
  }

  pub(crate) async fn load_main(
    module_map_rc: Rc<RefCell<ModuleMap>>,
    specifier: impl AsRef<str>,
  ) -> Result<RecursiveModuleLoad, Error> {
    let load =
      RecursiveModuleLoad::main(specifier.as_ref(), module_map_rc.clone());
    load.prepare().await?;
    Ok(load)
  }

  pub(crate) async fn load_side(
    module_map_rc: Rc<RefCell<ModuleMap>>,
    specifier: impl AsRef<str>,
  ) -> Result<RecursiveModuleLoad, Error> {
    let load =
      RecursiveModuleLoad::side(specifier.as_ref(), module_map_rc.clone());
    load.prepare().await?;
    Ok(load)
  }

  // Initiate loading of a module graph imported using `import()`.
  pub(crate) fn load_dynamic_import(
    module_map_rc: Rc<RefCell<ModuleMap>>,
    specifier: &str,
    referrer: &str,
    asserted_module_type: AssertedModuleType,
    resolver_handle: v8::Global<v8::PromiseResolver>,
  ) {
    let load = RecursiveModuleLoad::dynamic_import(
      specifier,
      referrer,
      asserted_module_type,
      module_map_rc.clone(),
    );
    module_map_rc
      .borrow_mut()
      .dynamic_import_map
      .insert(load.id, resolver_handle);

    let loader = module_map_rc.borrow().loader.clone();
    let resolve_result =
      loader.resolve(specifier, referrer, ResolutionKind::DynamicImport);
    let fut = match resolve_result {
      Ok(module_specifier) => {
        if module_map_rc
          .borrow()
          .is_registered(module_specifier, asserted_module_type)
        {
          async move { (load.id, Ok(load)) }.boxed_local()
        } else {
          async move { (load.id, load.prepare().await.map(|()| load)) }
            .boxed_local()
        }
      }
      Err(error) => async move { (load.id, Err(error)) }.boxed_local(),
    };
    module_map_rc
      .borrow_mut()
      .preparing_dynamic_imports
      .push(fut);
  }

  pub(crate) fn has_pending_dynamic_imports(&self) -> bool {
    !(self.preparing_dynamic_imports.is_empty()
      && self.pending_dynamic_imports.is_empty())
  }

  /// Returns the namespace object of a module.
  ///
  /// This is only available after module evaluation has completed.
  /// This function panics if module has not been instantiated.
  pub fn get_module_namespace(
    &self,
    scope: &mut v8::HandleScope,
    module_id: ModuleId,
  ) -> Result<v8::Global<v8::Object>, Error> {
    let module_handle =
      self.get_handle(module_id).expect("ModuleInfo not found");

    let module = module_handle.open(scope);

    if module.get_status() == v8::ModuleStatus::Errored {
      let exception = module.get_exception();
      return exception_to_err_result(scope, exception, false);
    }

    assert!(matches!(
      module.get_status(),
      v8::ModuleStatus::Instantiated | v8::ModuleStatus::Evaluated
    ));

    let module_namespace: v8::Local<v8::Object> =
      v8::Local::try_from(module.get_module_namespace())
        .map_err(|err: v8::DataError| generic_error(err.to_string()))?;

    Ok(v8::Global::new(scope, module_namespace))
  }

  /// Clear the module map, meant to be used after initializing extensions.
  /// Optionally pass a list of exceptions `(old_name, new_name)` representing
  /// specifiers which will be renamed and preserved in the module map.
  pub fn clear_module_map(&mut self, exceptions: &'static [&'static str]) {
    let handles = exceptions
      .iter()
      .map(|mod_name| (self.get_handle_by_name(mod_name).unwrap(), mod_name))
      .collect::<Vec<_>>();
    self.clear();
    for (handle, new_name) in handles {
      self.inject_handle(
        ModuleName::from_static(new_name),
        ModuleType::JavaScript,
        handle,
      )
    }
  }

  fn get_stalled_top_level_await_message_for_module(
    &self,
    scope: &mut v8::HandleScope,
    module_id: ModuleId,
  ) -> Vec<v8::Global<v8::Message>> {
    let module_handle = self.handles.get(module_id).unwrap();

    let module = v8::Local::new(scope, module_handle);
    let stalled = module.get_stalled_top_level_await_message(scope);
    let mut messages = vec![];
    for (_, message) in stalled {
      messages.push(v8::Global::new(scope, message));
    }
    messages
  }

  pub(crate) fn find_stalled_top_level_await(
    &self,
    scope: &mut v8::HandleScope,
  ) -> Vec<v8::Global<v8::Message>> {
    // First check if that's root module
    let root_module_id =
      self.info.iter().filter(|m| m.main).map(|m| m.id).next();

    if let Some(root_module_id) = root_module_id {
      let messages = self
        .get_stalled_top_level_await_message_for_module(scope, root_module_id);
      if !messages.is_empty() {
        return messages;
      }
    }

    // It wasn't a top module, so iterate over all modules and try to find
    // any with stalled top level await
    for module_id in 0..self.handles.len() {
      let messages =
        self.get_stalled_top_level_await_message_for_module(scope, module_id);
      if !messages.is_empty() {
        return messages;
      }
    }

    unreachable!()
  }
}

impl Default for ModuleMap {
  fn default() -> Self {
    Self::new(Rc::new(NoopModuleLoader))
  }
}

// Clippy thinks the return value doesn't need to be an Option, it's unaware
// of the mapping that MapFnFrom<F> does for ResolveModuleCallback.
#[allow(clippy::unnecessary_wraps)]
fn json_module_evaluation_steps<'a>(
  context: v8::Local<'a, v8::Context>,
  module: v8::Local<v8::Module>,
) -> Option<v8::Local<'a, v8::Value>> {
  // SAFETY: `CallbackScope` can be safely constructed from `Local<Context>`
  let scope = &mut unsafe { v8::CallbackScope::new(context) };
  let tc_scope = &mut v8::TryCatch::new(scope);
  let module_map = JsRuntime::module_map_from(tc_scope);

  let handle = v8::Global::<v8::Module>::new(tc_scope, module);
  let value_handle = module_map
    .borrow_mut()
    .json_value_store
    .remove(&handle)
    .unwrap();
  let value_local = v8::Local::new(tc_scope, value_handle);

  let name = v8::String::new(tc_scope, "default").unwrap();
  // This should never fail
  assert!(
    module.set_synthetic_module_export(tc_scope, name, value_local)
      == Some(true)
  );
  assert!(!tc_scope.has_caught());

  // Since TLA is active we need to return a promise.
  let resolver = v8::PromiseResolver::new(tc_scope).unwrap();
  let undefined = v8::undefined(tc_scope);
  resolver.resolve(tc_scope, undefined.into());
  Some(resolver.get_promise(tc_scope).into())
}

pub fn module_origin<'a>(
  s: &mut v8::HandleScope<'a>,
  resource_name: v8::Local<'a, v8::String>,
) -> v8::ScriptOrigin<'a> {
  let source_map_url = v8::String::empty(s);
  v8::ScriptOrigin::new(
    s,
    resource_name.into(),
    0,
    0,
    false,
    123,
    source_map_url.into(),
    true,
    false,
    true,
  )
}
