// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { notImplemented } = core.loadExtScript("ext:deno_node/_utils.ts");
const {
  op_vm_compile_function,
  op_vm_create_context,
  op_vm_create_context_without_contextify,
  op_vm_create_script,
  op_vm_is_context,
  op_vm_module_create_source_text_module,
  op_vm_module_create_synthetic_module,
  op_vm_module_evaluate,
  op_vm_module_get_exception,
  op_vm_module_get_identifier,
  op_vm_module_get_module_requests,
  op_vm_module_get_namespace,
  op_vm_module_get_status,
  op_vm_module_instantiate,
  op_vm_module_link,
  op_vm_module_set_synthetic_export,
  op_vm_script_create_cached_data,
  op_vm_script_get_source_map_url,
  op_vm_script_run_in_context,
} = core.ops;
const {
  validateArray,
  validateBoolean,
  validateBuffer,
  validateInt32,
  validateObject,
  validateOneOf,
  validateString,
  validateStringArray,
  validateUint32,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  ERR_MODULE_LINK_MISMATCH,
  ERR_VM_MODULE_ALREADY_LINKED,
  ERR_VM_MODULE_DIFFERENT_CONTEXT,
  ERR_VM_MODULE_NOT_MODULE,
  ERR_VM_MODULE_STATUS,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");

const {
  ArrayIsArray,
  ArrayPrototypeForEach,
  ArrayPrototypeIndexOf,
  ArrayPrototypeMap,
  ArrayPrototypePush,
  ArrayPrototypeSome,
  JSONStringify,
  ObjectAssign,
  ObjectFreeze,
  ObjectPrototypeHasOwnProperty,
  PromisePrototypeThen,
  PromiseReject,
  PromiseResolve,
  SafeMap,
  SafePromiseAll,
  Symbol,
} = primordials;

const kParsingContext = Symbol("script parsing context");

class Script {
  #inner;

  constructor(code, options = { __proto__: null }) {
    code = `${code}`;
    if (typeof options === "string") {
      options = { filename: options };
    } else {
      validateObject(options, "options");
    }

    const {
      filename = "evalmachine.<anonymous>",
      lineOffset = 0,
      columnOffset = 0,
      cachedData,
      produceCachedData = false,
      // importModuleDynamically,
      [kParsingContext]: parsingContext,
    } = options;

    validateString(filename, "options.filename");
    validateInt32(lineOffset, "options.lineOffset");
    validateInt32(columnOffset, "options.columnOffset");
    if (cachedData !== undefined) {
      validateBuffer(cachedData, "options.cachedData");
    }
    validateBoolean(produceCachedData, "options.produceCachedData");

    // const hostDefinedOptionId =
    //     getHostDefinedOptionId(importModuleDynamically, filename);

    const result = op_vm_create_script(
      code,
      filename,
      lineOffset,
      columnOffset,
      cachedData,
      produceCachedData,
      parsingContext,
    );
    this.#inner = result.value;
    this.cachedDataProduced = result.cached_data_produced;
    this.cachedDataRejected = result.cached_data_rejected;
    this.cachedData = result.cached_data
      ? Buffer.from(result.cached_data)
      : undefined;
  }

  #runInContext(contextifiedObject, options = { __proto__: null }) {
    validateObject(options, "options");

    let timeout = options.timeout;
    if (timeout === undefined) {
      timeout = -1;
    } else {
      validateUint32(timeout, "options.timeout", true);
    }

    const {
      displayErrors = true,
      breakOnSigint = false,
    } = options;

    validateBoolean(displayErrors, "options.displayErrors");
    validateBoolean(breakOnSigint, "options.breakOnSigint");

    //if (breakOnSigint && process.listenerCount('SIGINT') > 0) {
    //  return sigintHandlersWrap(super.runInContext, this, args);
    //}

    return op_vm_script_run_in_context(
      this.#inner,
      contextifiedObject,
      timeout,
      displayErrors,
      breakOnSigint,
    );
  }

  runInThisContext(options) {
    return this.#runInContext(null, options);
  }

  runInContext(contextifiedObject, options) {
    validateContext(contextifiedObject);
    return this.#runInContext(contextifiedObject, options);
  }

  runInNewContext(contextObject, options) {
    const context = createContext(contextObject, getContextOptions(options));
    return this.runInContext(context, options);
  }

  get sourceMapURL() {
    return op_vm_script_get_source_map_url(this.#inner);
  }

  createCachedData() {
    return Buffer.from(op_vm_script_create_cached_data(this.#inner));
  }
}

function validateContext(contextifiedObject) {
  if (!isContext(contextifiedObject)) {
    throw new ERR_INVALID_ARG_TYPE(
      "contextifiedObject",
      "vm.Context",
      contextifiedObject,
    );
  }
}

function getContextOptions(options) {
  if (!options) {
    return {};
  }
  const contextOptions = {
    name: options.contextName,
    origin: options.contextOrigin,
    codeGeneration: undefined,
    microtaskMode: options.microtaskMode,
  };
  if (contextOptions.name !== undefined) {
    validateString(contextOptions.name, "options.contextName");
  }
  if (contextOptions.origin !== undefined) {
    validateString(contextOptions.origin, "options.contextOrigin");
  }
  if (options.contextCodeGeneration !== undefined) {
    validateObject(
      options.contextCodeGeneration,
      "options.contextCodeGeneration",
    );
    const { strings, wasm } = options.contextCodeGeneration;
    if (strings !== undefined) {
      validateBoolean(strings, "options.contextCodeGeneration.strings");
    }
    if (wasm !== undefined) {
      validateBoolean(wasm, "options.contextCodeGeneration.wasm");
    }
    contextOptions.codeGeneration = { strings, wasm };
  }
  if (options.microtaskMode !== undefined) {
    validateString(options.microtaskMode, "options.microtaskMode");
  }
  return contextOptions;
}

let defaultContextNameIndex = 1;
function createContext(
  // deno-lint-ignore prefer-primordials
  contextObject = {},
  options = { __proto__: null },
) {
  if (contextObject === DONT_CONTEXTIFY) {
    validateObject(options, "options");

    const {
      name = `VM Context ${defaultContextNameIndex++}`,
      origin,
      codeGeneration,
      microtaskMode,
    } = options;

    validateString(name, "options.name");
    if (origin !== undefined) {
      validateString(origin, "options.origin");
    }
    if (codeGeneration !== undefined) {
      validateObject(codeGeneration, "options.codeGeneration");
    }

    let strings = true;
    let wasm = true;
    if (codeGeneration !== undefined) {
      ({ strings = true, wasm = true } = codeGeneration);
      validateBoolean(strings, "options.codeGeneration.strings");
      validateBoolean(wasm, "options.codeGeneration.wasm");
    }

    validateOneOf(microtaskMode, "options.microtaskMode", [
      "afterEvaluate",
      undefined,
    ]);
    const microtaskQueue = microtaskMode === "afterEvaluate";

    return op_vm_create_context_without_contextify(
      strings,
      wasm,
      microtaskQueue,
    );
  }

  if (isContext(contextObject)) {
    return contextObject;
  }

  validateObject(options, "options");

  const {
    name = `VM Context ${defaultContextNameIndex++}`,
    origin,
    codeGeneration,
    microtaskMode,
    // importModuleDynamically,
  } = options;

  validateString(name, "options.name");
  if (origin !== undefined) {
    validateString(origin, "options.origin");
  }
  if (codeGeneration !== undefined) {
    validateObject(codeGeneration, "options.codeGeneration");
  }

  let strings = true;
  let wasm = true;
  if (codeGeneration !== undefined) {
    ({ strings = true, wasm = true } = codeGeneration);
    validateBoolean(strings, "options.codeGeneration.strings");
    validateBoolean(wasm, "options.codeGeneration.wasm");
  }

  validateOneOf(microtaskMode, "options.microtaskMode", [
    "afterEvaluate",
    undefined,
  ]);
  const microtaskQueue = microtaskMode === "afterEvaluate";

  // const hostDefinedOptionId =
  //   getHostDefinedOptionId(importModuleDynamically, name);

  op_vm_create_context(
    contextObject,
    name,
    origin,
    strings,
    wasm,
    microtaskQueue,
  );
  // Register the context scope callback after the context was initialized.
  // registerImportModuleDynamically(contextObject, importModuleDynamically);
  return contextObject;
}

function createScript(code, options) {
  return new Script(code, options);
}

function runInContext(code, contextifiedObject, options) {
  validateContext(contextifiedObject);
  if (typeof options === "string") {
    options = {
      filename: options,
      [kParsingContext]: contextifiedObject,
    };
  } else {
    options = {
      ...options,
      [kParsingContext]: contextifiedObject,
    };
  }
  return createScript(code, options)
    .runInContext(contextifiedObject, options);
}

function runInNewContext(code, contextObject, options) {
  if (typeof options === "string") {
    options = { filename: options };
  }
  contextObject = createContext(contextObject, getContextOptions(options));
  options = { ...options, [kParsingContext]: contextObject };
  return createScript(code, options).runInNewContext(contextObject, options);
}

function runInThisContext(code, options) {
  if (typeof options === "string") {
    options = { filename: options };
  }
  return createScript(code, options).runInThisContext(options);
}

function isContext(object) {
  validateObject(object, "object", { allowArray: true });
  return op_vm_is_context(object);
}

function compileFunction(code, params, options = { __proto__: null }) {
  validateString(code, "code");
  if (params !== undefined) {
    validateStringArray(params, "params");
  }
  validateObject(options, "options");
  const {
    filename = "",
    columnOffset = 0,
    lineOffset = 0,
    cachedData = undefined,
    produceCachedData = false,
    parsingContext = undefined,
    contextExtensions = [],
    // importModuleDynamically,
  } = options;

  validateString(filename, "options.filename");
  validateInt32(columnOffset, "options.columnOffset");
  validateInt32(lineOffset, "options.lineOffset");
  if (cachedData !== undefined) {
    validateBuffer(cachedData, "options.cachedData");
  }
  validateBoolean(produceCachedData, "options.produceCachedData");
  if (parsingContext !== undefined) {
    if (
      typeof parsingContext !== "object" ||
      parsingContext === null ||
      !isContext(parsingContext)
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.parsingContext",
        "Context",
        parsingContext,
      );
    }
  }
  validateArray(contextExtensions, "options.contextExtensions");
  ArrayPrototypeForEach(contextExtensions, (extension, i) => {
    const name = `options.contextExtensions[${i}]`;
    validateObject(extension, name, { nullable: true });
  });

  // const hostDefinedOptionId =
  //     getHostDefinedOptionId(importModuleDynamically, filename);

  const result = op_vm_compile_function(
    code,
    filename,
    lineOffset,
    columnOffset,
    cachedData,
    produceCachedData,
    parsingContext,
    contextExtensions,
    params,
  );

  result.value.cachedDataProduced = result.cached_data_produced;
  result.value.cachedDataRejected = result.cached_data_rejected;
  result.value.cachedData = result.cached_data
    ? Buffer.from(result.cached_data)
    : undefined;

  return result.value;
}

function measureMemory(_options) {
  notImplemented("measureMemory");
}

const USE_MAIN_CONTEXT_DEFAULT_LOADER = Symbol(
  "USE_MAIN_CONTEXT_DEFAULT_LOADER",
);
const DONT_CONTEXTIFY = Symbol("DONT_CONTEXTIFY");

const constants = {
  __proto__: null,
  USE_MAIN_CONTEXT_DEFAULT_LOADER,
  DONT_CONTEXTIFY,
};

ObjectFreeze(constants);

// vm.Module / vm.SourceTextModule (experimental in Node).
//
// Status integers map to the V8 ModuleStatus enum:
//   0=Uninstantiated, 1=Instantiating, 2=Instantiated,
//   3=Evaluating, 4=Evaluated, 5=Errored
const STATUS_NAMES = [
  "unlinked",
  "linking",
  "linked",
  "evaluating",
  "evaluated",
  "errored",
];

const kWrap = Symbol("kWrap");
const kContext = Symbol("kContext");
const kLink = Symbol("kLink");
const kLinkingStatus = Symbol("kLinkingStatus");
const kModuleRequests = Symbol("kModuleRequests");
const kDependencySpecifiers = Symbol("kDependencySpecifiers");
let defaultModuleIdIndex = 0;

function isModule(object) {
  return typeof object === "object" && object !== null &&
    ObjectPrototypeHasOwnProperty(object, kWrap);
}

function buildModuleRequests(wrap) {
  const raw = op_vm_module_get_module_requests(wrap);
  const out = [];
  for (let i = 0; i < raw.length; i++) {
    const r = raw[i];
    const attrs = { __proto__: null };
    // Object.assign on a null-proto target copies enumerable string keys.
    ObjectAssign(attrs, r.attributes);
    ObjectFreeze(attrs);
    out[i] = ObjectFreeze({
      __proto__: null,
      specifier: r.specifier,
      attributes: attrs,
      phase: r.phase,
    });
  }
  return ObjectFreeze(out);
}

class Module {
  constructor() {
    if (new.target === Module) {
      throw new ERR_INVALID_ARG_TYPE(
        "this",
        "vm.SourceTextModule | vm.SyntheticModule",
        this,
      );
    }
    this[kLinkingStatus] = null;
  }

  get identifier() {
    return op_vm_module_get_identifier(this[kWrap]);
  }

  get context() {
    return this[kContext];
  }

  get namespace() {
    const status = op_vm_module_get_status(this[kWrap]);
    if (status < 2) {
      throw new ERR_VM_MODULE_STATUS("must not be unlinked or linking");
    }
    return op_vm_module_get_namespace(this[kWrap]);
  }

  get status() {
    if (this[kLinkingStatus] !== null) {
      return this[kLinkingStatus];
    }
    return STATUS_NAMES[op_vm_module_get_status(this[kWrap])];
  }

  get error() {
    if (this.status !== "errored") {
      throw new ERR_VM_MODULE_STATUS("must be errored");
    }
    return op_vm_module_get_exception(this[kWrap]);
  }

  link(linker) {
    if (typeof linker !== "function") {
      throw new ERR_INVALID_ARG_TYPE("linker", "function", linker);
    }
    if (this.status !== "unlinked") {
      throw new ERR_VM_MODULE_ALREADY_LINKED();
    }
    this[kLinkingStatus] = "linking";
    return PromisePrototypeThen(this[kLink](linker), (v) => {
      this[kLinkingStatus] = null;
      return v;
    }, (e) => {
      this[kLinkingStatus] = null;
      throw e;
    });
  }

  // TODO(divybot): cyclic imports via `link(linker)` not yet supported.
  // Single-pass linking makes instantiation order depend on linker
  // resolution order: in a diamond (A imports B and C; B also imports C)
  // C is instantiated when B's kLink finishes, so by the time A's
  // instantiate runs, B and C are already instantiated. V8 tolerates
  // re-instantiating an instantiated module as a no-op, but a Node-style
  // two-phase resolve+instantiate would be needed for true cyclic-import
  // support. The newer `linkRequests` + `instantiate` API does support
  // cycles since linking is decoupled from instantiation.
  async [kLink](linker) {
    const requests = this[kModuleRequests];
    const specifiers = [];
    const modules = [];
    const linkerPromises = [];
    for (let i = 0; i < requests.length; i++) {
      const { specifier, attributes } = requests[i];
      ArrayPrototypePush(specifiers, specifier);
      const p = PromiseResolve(
        linker(specifier, this, { attributes, assert: attributes }),
      );
      ArrayPrototypePush(linkerPromises, p);
    }
    const resolvedModules = await SafePromiseAll(linkerPromises);
    for (let i = 0; i < resolvedModules.length; i++) {
      const m = resolvedModules[i];
      if (!isModule(m)) {
        throw new ERR_VM_MODULE_NOT_MODULE();
      }
      if (m.context !== this[kContext]) {
        throw new ERR_VM_MODULE_DIFFERENT_CONTEXT();
      }
      if (m.status === "unlinked") {
        await m[kLink](linker);
      }
      ArrayPrototypePush(modules, m[kWrap]);
    }

    op_vm_module_link(this[kWrap], specifiers, modules);
    op_vm_module_instantiate(this[kWrap]);
  }

  evaluate(options = { __proto__: null }) {
    try {
      validateObject(options, "options");
      const status = op_vm_module_get_status(this[kWrap]);
      // Allow evaluate from linked (2), evaluating (3), evaluated (4), errored (5).
      if (status < 2) {
        throw new ERR_VM_MODULE_STATUS(
          "must be one of linked, evaluated, or errored",
        );
      }
      // Return the V8 Promise directly so that synthetic modules with sync
      // evaluation steps produce a synchronously-resolved Promise (matching
      // Node's behavior).
      return op_vm_module_evaluate(this[kWrap]);
    } catch (e) {
      return PromiseReject(e);
    }
  }
}

class SourceTextModule extends Module {
  constructor(sourceText, options = { __proto__: null }) {
    super();
    if (typeof sourceText !== "string") {
      throw new ERR_INVALID_ARG_TYPE("sourceText", "string", sourceText);
    }
    validateObject(options, "options");
    const {
      identifier = `vm:module(${defaultModuleIdIndex++})`,
      context,
      lineOffset = 0,
      columnOffset = 0,
    } = options;
    if (context !== undefined) {
      validateContext(context);
    }
    validateString(identifier, "options.identifier");
    validateInt32(lineOffset, "options.lineOffset");
    validateInt32(columnOffset, "options.columnOffset");

    this[kContext] = context;
    this[kWrap] = op_vm_module_create_source_text_module(
      sourceText,
      identifier,
      lineOffset,
      columnOffset,
      context,
    );
    this[kModuleRequests] = buildModuleRequests(this[kWrap]);
    this[kDependencySpecifiers] = undefined;
  }

  get moduleRequests() {
    return this[kModuleRequests];
  }

  get dependencySpecifiers() {
    if (this[kDependencySpecifiers] === undefined) {
      this[kDependencySpecifiers] = ObjectFreeze(
        ArrayPrototypeMap(this[kModuleRequests], (r) => r.specifier),
      );
    }
    return this[kDependencySpecifiers];
  }

  linkRequests(modules) {
    if (this.status !== "unlinked") {
      throw new ERR_VM_MODULE_STATUS("must be unlinked");
    }
    validateArray(modules, "modules");
    const requests = this[kModuleRequests];
    if (modules.length !== requests.length) {
      throw new ERR_MODULE_LINK_MISMATCH(
        `Expected ${requests.length} modules, got ${modules.length}`,
      );
    }
    // Validate each provided module first (type + context), then check for
    // cache-key collisions: two requests sharing (specifier, attributes)
    // must map to the same module instance, matching Node's V8 binding
    // behavior. We use this single pass to keep the error precedence
    // identical to Node's tests.
    const seen = new SafeMap();
    const specifiers = [];
    const wraps = [];
    for (let i = 0; i < modules.length; i++) {
      const m = modules[i];
      if (!isModule(m)) {
        throw new ERR_VM_MODULE_NOT_MODULE();
      }
      if (m.context !== this[kContext]) {
        throw new ERR_VM_MODULE_DIFFERENT_CONTEXT();
      }
      const { specifier, attributes } = requests[i];
      const key = `${specifier}\0${JSONStringify(attributes)}`;
      if (seen.has(key)) {
        if (seen.get(key) !== m) {
          throw new ERR_MODULE_LINK_MISMATCH(
            `Different modules linked to the same cache key '${specifier}'`,
          );
        }
      } else {
        seen.set(key, m);
      }
      ArrayPrototypePush(specifiers, specifier);
      ArrayPrototypePush(wraps, m[kWrap]);
    }

    op_vm_module_link(this[kWrap], specifiers, wraps);
  }

  instantiate() {
    op_vm_module_instantiate(this[kWrap]);
  }
}

class SyntheticModule extends Module {
  constructor(exportNames, evaluateCallback, options = { __proto__: null }) {
    super();
    if (
      !ArrayIsArray(exportNames) ||
      ArrayPrototypeSome(exportNames, (e) => typeof e !== "string")
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "exportNames",
        "Array of unique strings",
        exportNames,
      );
    }
    ArrayPrototypeForEach(exportNames, (name, i) => {
      if (ArrayPrototypeIndexOf(exportNames, name, i + 1) !== -1) {
        throw new ERR_INVALID_ARG_VALUE(
          `exportNames.${name}`,
          name,
          "is duplicated",
        );
      }
    });
    if (typeof evaluateCallback !== "function") {
      throw new ERR_INVALID_ARG_TYPE(
        "evaluateCallback",
        "function",
        evaluateCallback,
      );
    }
    validateObject(options, "options");
    const {
      identifier = `vm:module(${defaultModuleIdIndex++})`,
      context,
    } = options;
    if (context !== undefined) {
      validateContext(context);
    }
    validateString(identifier, "options.identifier");

    this[kContext] = context;
    this[kWrap] = op_vm_module_create_synthetic_module(
      identifier,
      exportNames,
      context,
      evaluateCallback,
      this,
    );
    // Synthetic modules have no dependencies; instantiate immediately so
    // the module enters the `linked` state and is ready for evaluation.
    op_vm_module_instantiate(this[kWrap]);
  }

  link() {
    // No-op for synthetic modules. The base `Module.link` would otherwise
    // throw ERR_VM_MODULE_ALREADY_LINKED because the constructor already
    // instantiated us.
  }

  setExport(name, value) {
    validateString(name, "name");
    const status = op_vm_module_get_status(this[kWrap]);
    if (status < 2) {
      throw new ERR_VM_MODULE_STATUS("must be linked");
    }
    op_vm_module_set_synthetic_export(this[kWrap], name, value);
  }
}

return {
  default: {
    Module,
    Script,
    SourceTextModule,
    SyntheticModule,
    constants,
    createContext,
    createScript,
    runInContext,
    runInNewContext,
    runInThisContext,
    isContext,
    compileFunction,
    measureMemory,
  },
  Module,
  Script,
  SourceTextModule,
  SyntheticModule,
  constants,
  createContext,
  createScript,
  runInContext,
  runInNewContext,
  runInThisContext,
  isContext,
  compileFunction,
  measureMemory,
};
})();
