// Copyright 2018-2026 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import {
  op_bundle,
  op_bundle_plugin_finish,
  op_bundle_plugin_next,
  op_bundle_plugin_respond,
  op_bundle_plugin_start,
} from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
const { TextDecoder } = core.loadExtScript("ext:deno_web/08_text_encoding.js");

const {
  SafeArrayIterator,
  Uint8Array,
  ObjectPrototypeIsPrototypeOf,
  ArrayPrototypePush,
  RegExpPrototype,
  RegExpPrototypeTest,
  String,
  TypeError,
} = primordials;

function isRegExp(value) {
  return ObjectPrototypeIsPrototypeOf(RegExpPrototype, value);
}

const decoder = new TextDecoder();

// A resolve/load/transform callback registered by a plugin, paired with the
// filter it should be tested against.
class RegisteredHook {
  filter;
  callback;
  constructor(filter, callback) {
    this.filter = filter;
    this.callback = callback;
  }
}

// Collects the hooks a plugin registers via its `setup(build)` call.
class PluginRegistry {
  onResolve = [];
  onLoad = [];
  onTransform = [];
}

function registerHook(hooks, hookName, options, callback) {
  if (!options || !isRegExp(options.filter)) {
    throw new TypeError(`${hookName} requires a \`filter\` RegExp`);
  }
  ArrayPrototypePush(hooks, new RegisteredHook(options.filter, callback));
}

// Builds the `build` object passed to each plugin's `setup`, recording
// registered hooks into `registry`.
function makePluginBuild(registry) {
  return {
    onResolve: (options, callback) =>
      registerHook(registry.onResolve, "onResolve", options, callback),
    onLoad: (options, callback) =>
      registerHook(registry.onLoad, "onLoad", options, callback),
    onTransform: (options, callback) =>
      registerHook(registry.onTransform, "onTransform", options, callback),
  };
}

// Runs the first matching hook (in registration order) whose filter matches
// `subject`, returning its result. Returns null when none match or all defer.
async function runHooks(hooks, subject, args) {
  for (const hook of new SafeArrayIterator(hooks)) {
    if (!RegExpPrototypeTest(hook.filter, subject)) {
      continue;
    }
    const result = await hook.callback(args);
    if (result != null) {
      return result;
    }
  }
  return null;
}

async function dispatchHook(registry, request) {
  switch (request.hook) {
    case "resolve": {
      const result = await runHooks(registry.onResolve, request.specifier, {
        specifier: request.specifier,
        importer: request.importer,
        kind: request.kind,
      });
      if (result == null) return null;
      return { id: result.id, external: result.external };
    }
    case "load": {
      const result = await runHooks(registry.onLoad, request.id, {
        id: request.id,
      });
      if (result == null) return null;
      return { code: result.code, loader: result.loader };
    }
    case "transform": {
      const result = await runHooks(registry.onTransform, request.id, {
        id: request.id,
        code: request.code,
      });
      if (result == null) return null;
      return { code: result.code };
    }
    default:
      return null;
  }
}

// Pumps hook requests from the native bundler until the build finishes,
// running the registered plugin chain for each one.
async function runWithPlugins(options, plugins) {
  const registry = new PluginRegistry();
  const build = makePluginBuild(registry);
  for (const plugin of new SafeArrayIterator(plugins)) {
    await plugin.setup(build);
  }

  // The native side reads `options` without the (non-serializable) plugins.
  const { plugins: _plugins, ...nativeOptions } = options;
  const sessionId = await op_bundle_plugin_start(nativeOptions);

  while (true) {
    const request = await op_bundle_plugin_next(sessionId);
    if (request == null) {
      break;
    }
    let response = null;
    try {
      response = await dispatchHook(registry, request);
    } catch (err) {
      op_bundle_plugin_respond(sessionId, request.requestId, {
        error: err?.message ?? String(err),
      });
      continue;
    }
    op_bundle_plugin_respond(sessionId, request.requestId, response);
  }

  return op_bundle_plugin_finish(sessionId);
}

export async function bundle(
  options: Deno.bundle.Options,
): Promise<Deno.bundle.Result> {
  const hasPlugins = options.plugins != null && options.plugins.length > 0;
  const result = {
    success: false,
    ...(hasPlugins
      ? await runWithPlugins(options, options.plugins)
      : await op_bundle(options)),
  };
  result.success = result.errors.length === 0;

  for (
    const f of new SafeArrayIterator(
      // deno-lint-ignore no-explicit-any
      result.outputFiles as any ?? [],
    )
  ) {
    // deno-lint-ignore no-explicit-any
    const file = f as any;
    if (file.contents?.length === 0) {
      delete file.contents;
      file.text = () => "";
    } else {
      if (!ObjectPrototypeIsPrototypeOf(Uint8Array, file.contents)) {
        file.contents = new Uint8Array(file.contents);
      }
      file.text = () => decoder.decode(file.contents ?? "");
    }
  }
  if (result.outputFiles?.length === 0) {
    delete result.outputFiles;
  }
  return result;
}
