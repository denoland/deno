// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { Buffer } from "node:buffer";
import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  op_vm_compile_function,
  op_vm_create_context,
  op_vm_create_context_without_contextify,
  op_vm_create_script,
  op_vm_is_context,
  op_vm_module_create_source_text_module,
  op_vm_module_evaluate,
  op_vm_module_get_exception,
  op_vm_module_get_identifier,
  op_vm_module_get_module_requests,
  op_vm_module_get_namespace,
  op_vm_module_get_status,
  op_vm_module_instantiate,
  op_vm_module_link,
  op_vm_script_create_cached_data,
  op_vm_script_get_source_map_url,
  op_vm_script_run_in_context,
} from "ext:core/ops";
import {
  validateArray,
  validateBoolean,
  validateBuffer,
  validateInt32,
  validateObject,
  validateOneOf,
  validateString,
  validateStringArray,
  validateUint32,
} from "ext:deno_node/internal/validators.mjs";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_VM_MODULE_ALREADY_LINKED,
  ERR_VM_MODULE_DIFFERENT_CONTEXT,
  ERR_VM_MODULE_NOT_MODULE,
  ERR_VM_MODULE_STATUS,
} from "ext:deno_node/internal/errors.ts";

import { primordials } from "ext:core/mod.js";

const {
  ArrayPrototypeForEach,
  ArrayPrototypePush,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  PromisePrototypeThen,
  PromiseResolve,
  SafePromiseAll,
  Symbol,
} = primordials;

const kParsingContext = Symbol("script parsing context");

export class Script {
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
export function createContext(
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

export function createScript(code, options) {
  return new Script(code, options);
}

export function runInContext(code, contextifiedObject, options) {
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

export function runInNewContext(code, contextObject, options) {
  if (typeof options === "string") {
    options = { filename: options };
  }
  contextObject = createContext(contextObject, getContextOptions(options));
  options = { ...options, [kParsingContext]: contextObject };
  return createScript(code, options).runInNewContext(contextObject, options);
}

export function runInThisContext(code, options) {
  if (typeof options === "string") {
    options = { filename: options };
  }
  return createScript(code, options).runInThisContext(options);
}

export function isContext(object) {
  validateObject(object, "object", { allowArray: true });
  return op_vm_is_context(object);
}

export function compileFunction(code, params, options = { __proto__: null }) {
  validateString(code, "code");
  if (params !== undefined) {
    validateStringArray(params, "params");
  }
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

export function measureMemory(_options) {
  notImplemented("measureMemory");
}

const USE_MAIN_CONTEXT_DEFAULT_LOADER = Symbol(
  "USE_MAIN_CONTEXT_DEFAULT_LOADER",
);
const DONT_CONTEXTIFY = Symbol("DONT_CONTEXTIFY");

export const constants = {
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
let defaultModuleIdIndex = 0;

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

  async [kLink](linker) {
    const requests = op_vm_module_get_module_requests(this[kWrap]);
    const specifiers = [];
    const modules = [];
    const linkerPromises = [];
    for (let i = 0; i < requests.length; i++) {
      const specifier = requests[i];
      ArrayPrototypePush(specifiers, specifier);
      const p = PromiseResolve(
        linker(specifier, this, { attributes: {} }),
      );
      ArrayPrototypePush(linkerPromises, p);
    }
    const resolvedModules = await SafePromiseAll(linkerPromises);
    for (let i = 0; i < resolvedModules.length; i++) {
      const m = resolvedModules[i];
      if (!ObjectPrototypeIsPrototypeOf(Module.prototype, m)) {
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

  async evaluate(_options = { __proto__: null }) {
    const status = op_vm_module_get_status(this[kWrap]);
    // Allow evaluate from linked (2), evaluating (3), evaluated (4), errored (5).
    if (status < 2) {
      throw new ERR_VM_MODULE_STATUS(
        "must be one of linked, evaluated, or errored",
      );
    }
    const result = op_vm_module_evaluate(this[kWrap]);
    return await result;
  }
}

export class SourceTextModule extends Module {
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
  }
}

export default {
  Script,
  SourceTextModule,
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
