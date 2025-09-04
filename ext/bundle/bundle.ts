// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import { op_bundle } from "ext:core/ops";
import { primordials } from "ext:core/mod.js";
import { TextDecoder } from "ext:deno_web/08_text_encoding.js";

const { SafeArrayIterator, Uint8Array, ObjectPrototypeIsPrototypeOf } =
  primordials;

const decoder = new TextDecoder();

type HookTypes = {
  [Hook.onStart]: Deno.bundle.OnStartResult;
  [Hook.onResolve]: Deno.bundle.OnResolveResult;
  [Hook.onLoad]: Deno.bundle.OnLoadResult;
  [Hook.onEnd]: Deno.bundle.OnEndResult;
  [Hook.onDispose]: void;
};

type PluginInfo = {
  id: number;
  name: string;
  onResolve: null | {
    options: Deno.bundle.OnResolveOptions;
    callback: Parameters<Deno.bundle.PluginBuild["onResolve"]>[1];
  };
  onLoad: null | {
    options: Deno.bundle.OnLoadOptions;
    callback: Parameters<Deno.bundle.PluginBuild["onLoad"]>[1];
  };
  onStart: null | {
    callback: Parameters<Deno.bundle.PluginBuild["onStart"]>[0];
  };
  onEnd: null | {
    callback: Parameters<Deno.bundle.PluginBuild["onEnd"]>[0];
  };
  onDispose: null | {
    callback: Parameters<Deno.bundle.PluginBuild["onDispose"]>[0];
  };
};

async function collectPluginInfo(
  options: Deno.bundle.Options,
): Promise<[PluginInfo[], Deno.bundle.PluginBuild | undefined]> {
  const plugins: PluginInfo[] = [];
  if (options.plugins) {
    let info: Omit<PluginInfo, "id" | "name"> = {
      onResolve: null,
      onLoad: null,
      onStart: null,
      onEnd: null,
      onDispose: null,
    };

    const pluginBuild: Deno.bundle.PluginBuild = {
      initialOptions: options,
      resolve: (_path, _options) => {
        throw new Error("Not implemented");
      },
      onEnd: (callback) => {
        info.onEnd = { callback };
      },
      onResolve: (options, callback) => {
        info.onResolve = { callback, options };
      },
      onLoad: (options, callback) => {
        info.onLoad = { callback, options };
      },
      onStart: (callback) => {
        info.onStart = { callback };
      },
      onDispose: (callback) => {
        info.onDispose = { callback };
      },
    };
    for (const plugin of options.plugins ?? []) {
      await plugin.setup(pluginBuild);
      plugins.push({
        id: plugins.length,
        name: plugin.name,
        onResolve: info.onResolve,
        onLoad: info.onLoad,
        onStart: info.onStart,
        onEnd: info.onEnd,
        onDispose: info.onDispose,
      });
      info = {
        onResolve: null,
        onLoad: null,
        onStart: null,
        onEnd: null,
        onDispose: null,
      };
    }
    return [plugins, pluginBuild];
  }
  return [[], undefined];
}

interface RustPluginInfo {
  name: string;
  id: number;
  onStart: boolean;
  onEnd: boolean;
  onResolve:
    | (Omit<Deno.bundle.OnResolveOptions, "filter"> & { filter: string })
    | null;
  onLoad:
    | (Omit<Deno.bundle.OnLoadOptions, "filter"> & { filter: string })
    | null;
  onDispose: boolean;
}

function onResolveToRustPluginInfo(
  onResolve: Deno.bundle.OnResolveOptions | undefined,
): RustPluginInfo["onResolve"] {
  if (!onResolve) return null;
  return {
    ...onResolve,
    filter: onResolve.filter.source,
  };
}

function onLoadToRustPluginInfo(
  onLoad: Deno.bundle.OnLoadOptions | undefined,
): RustPluginInfo["onLoad"] {
  if (!onLoad) return null;
  return {
    ...onLoad,
    filter: onLoad.filter.source,
  };
}

function toRustPluginInfo(plugins: PluginInfo[]): RustPluginInfo[] {
  return plugins.map((plugin) => ({
    name: plugin.name,
    id: plugin.id,
    onStart: !!plugin.onStart,
    onEnd: !!plugin.onEnd,
    onResolve: onResolveToRustPluginInfo(plugin.onResolve?.options),
    onLoad: onLoadToRustPluginInfo(plugin.onLoad?.options),
    onDispose: !!plugin.onDispose,
  }));
}

const enum HookType {
  first = 0,
  sequential = 1,
}

const enum Hook {
  onStart = 0,
  onResolve = 1,
  onLoad = 2,
  onEnd = 3,
  onDispose = 4,
}

function exhaustive(_x: never): never {
  throw new Error("Unreachable");
}

function getHookName(
  hook: Hook,
): "onStart" | "onResolve" | "onLoad" | "onEnd" | "onDispose" {
  switch (hook) {
    case Hook.onStart:
      return "onStart";
    case Hook.onResolve:
      return "onResolve";
    case Hook.onLoad:
      return "onLoad";
    case Hook.onEnd:
      return "onEnd";
    case Hook.onDispose:
      return "onDispose";
    default:
      exhaustive(hook);
  }
}

function defaultResult<H extends Hook>(hook: H): HookTypes[H] {
  switch (hook) {
    case Hook.onStart:
      return { errors: [], warnings: [] } as unknown as HookTypes[H];
    case Hook.onResolve:
      return null as unknown as HookTypes[H];
    case Hook.onLoad:
      return null as unknown as HookTypes[H];
    case Hook.onEnd:
      return { errors: [], warnings: [] } as unknown as HookTypes[H];
    case Hook.onDispose:
      return null as unknown as HookTypes[H];
    default:
      exhaustive(hook);
  }
}

function combineResult(
  hook: Hook,
  currentValue: HookTypes[Hook] | null,
  newValue: HookTypes[Hook],
): HookTypes[Hook] {
  switch (hook) {
    case Hook.onStart:
    case Hook.onEnd:
      return {
        errors: [...(currentValue?.errors ?? []), ...(newValue?.errors ?? [])],
        warnings: [
          ...(currentValue?.warnings ?? []),
          ...(newValue?.warnings ?? []),
        ],
      } as HookTypes[Hook];
    case Hook.onResolve:
    case Hook.onLoad:
      return newValue;
    case Hook.onDispose:
      return undefined as HookTypes[Hook];
    default:
      exhaustive(hook);
  }
}

function makePluginExecutor(
  plugins: PluginInfo[],
  pluginBuild: Deno.bundle.PluginBuild,
) {
  if (!plugins.length) {
    return (..._args: unknown[]) => Promise.resolve();
  }

  return async <H extends Hook>(
    hook: H,
    type: HookType,
    ids: number[],
    sender: { sendResult: (res: unknown) => void },
    resolve: {
      resolve: (
        path: string,
        options?: Deno.bundle.ResolveOptions,
      ) => Promise<Deno.bundle.ResolveResult>;
    },
    args: unknown[],
  ): Promise<void> => {
    pluginBuild.resolve = resolve.resolve.bind(resolve);
    let result: HookTypes[H] = defaultResult(hook) as HookTypes[H];
    const hookName = getHookName(hook);
    for (const id of ids) {
      // deno-lint-ignore no-explicit-any
      const plugin = plugins[id] as any;
      if (plugin[hookName]) {
        const newResult = await plugin[hookName].callback(...args);
        if (type === HookType.first && newResult) {
          result = newResult as HookTypes[H];
          sender.sendResult({ pluginId: id, result });
          return;
        } else if (type === HookType.sequential) {
          result = combineResult(
            hook,
            result as HookTypes[Hook],
            newResult as HookTypes[Hook],
          ) as HookTypes[H];
          continue;
        }
      }
    }
    sender.sendResult({ pluginId: null, result });
  };
}

export async function bundle(
  options: Deno.bundle.Options,
): Promise<Deno.bundle.Result> {
  const [plugins, pluginBuild] = await collectPluginInfo(options);
  const forRust = toRustPluginInfo(plugins);
  const result = {
    success: false,
    ...await op_bundle(
      options,
      forRust.length > 0 ? forRust : null,
      forRust.length > 0 ? makePluginExecutor(plugins, pluginBuild!) : null,
    ),
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
