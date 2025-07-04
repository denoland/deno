// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference path="../../cli/tsc/dts/lib.deno.unstable.d.ts" />
import { op_bundle } from "ext:core/ops";

declare function op_bundle(
  options: Omit<Deno.bundle.Options, "plugins">,
  plugins: RustPluginInfo[] | null,
): Promise<Omit<Deno.bundle.Result, "success">>;

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

function makePluginExecutor(
  options: Deno.bundle.Options,
  plugins: PluginInfo[],
) {
  if (!plugins.length) {
    return (..._args: any) => null;
  }
  
  return (hookName: "onResolve" | "onLoad" | "onStart" | "onEnd" | "onDispose", ...args: any[]) => {
  }
}

export async function bundle(
  options: Deno.bundle.Options,
): Promise<Deno.bundle.Result> {
  const plugins = await collectPluginInfo(options);

  const forRust = toRustPluginInfo(plugins);

  const result = {
    success: false,
    ...await op_bundle(options, forRust.length > 0 ? forRust : null),
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
