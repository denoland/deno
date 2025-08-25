// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import { op_bundle, PluginExecResultSenderWrapper } from "ext:core/ops";

declare function op_bundle(
  options: Omit<Deno.bundle.Options, "plugins">,
  plugins: RustPluginInfo[] | null,
  pluginExecutor:
    | ((
      hook: Hook,
      type: HookType,
      ids: number[],
      sender: PluginExecResultSenderWrapper,
      ...args: any[]
    ) => Promise<void>)
    | null,
): Promise<Omit<Deno.bundle.Result, "success">>;

interface PluginExecResultSenderWrapper {
  sendResult: (result: any) => void;
}

interface PluginInfo {
  id: number;
  name: string;
  onResolve: null | {
    callback: Parameters<Deno.bundle.PluginBuild["onResolve"]>[1];
    options: Deno.bundle.OnResolveOptions;
  };
  onLoad: null | {
    callback: Parameters<Deno.bundle.PluginBuild["onLoad"]>[1];
    options: Deno.bundle.OnLoadOptions;
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
}
async function collectPluginInfo(
  options: Deno.bundle.Options,
): Promise<PluginInfo[]> {
  const plugins = [];
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
  }
  return plugins;
}

interface RustPluginInfo {
  name: string;
  id: number;
  onStart: boolean;
  onEnd: boolean;
  onResolve: Deno.bundle.OnResolveOptions | null;
  onLoad: Deno.bundle.OnLoadOptions | null;
  onDispose: boolean;
}

function toRustPluginInfo(plugins: PluginInfo[]): RustPluginInfo[] {
  return plugins.map((plugin) => ({
    name: plugin.name,
    id: plugin.id,
    onStart: !!plugin.onStart,
    onEnd: !!plugin.onEnd,
    onResolve: plugin.onResolve?.options ?? null,
    onLoad: plugin.onLoad?.options ?? null,
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
  throw new Error("Unreachable, but got: " + _x);
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

function makePluginExecutor(
  // options: Deno.bundle.Options,
  plugins: PluginInfo[],
) {
  if (!plugins.length) {
    return (..._args: any) => Promise.resolve();
  }

  return async (
    hook: Hook,
    type: HookType,
    ids: number[],
    sender: PluginExecResultSenderWrapper,
    ...args: any[]
  ): Promise<void> => {
    let result = null;
    const hookName = getHookName(hook);
    for (const id of ids) {
      const plugin = plugins[id];
      if (plugin[hookName]) {
        // @ts-ignore aaa
        result = await plugin[hookName].callback(...args);
        if (type === HookType.first && result) {
          sender.sendResult({ pluginId: id, result });
          return;
        } else if (type === HookType.sequential) {
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
  const plugins = await collectPluginInfo(options);

  const forRust = toRustPluginInfo(plugins);

  const result = {
    success: false,
    ...await op_bundle(
      options,
      forRust.length > 0 ? forRust : null,
      forRust.length > 0 ? makePluginExecutor(plugins) : null,
    ),
  };
  result.success = result.errors.length === 0;

  for (const file of result.outputFiles ?? []) {
    if (file.contents?.length === 0) {
      delete file.contents;
    }
  }
  if (result.outputFiles?.length === 0) {
    delete result.outputFiles;
  }
  return result;
}
