// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { createGraph, ModuleGraphJson, ModuleJson } from "deno_graph";

/**
 * Checks for circular dependencies in the std submodules.
 *
 * When run with `--graph` it will output a graphviz graph in dot language.
 */

type DepState = "ready" | "not ready" | "needs clean up" | "deprecated";
type Dep = {
  name: string;
  set: Set<string>;
  state: DepState;
};

const root = new URL("../", import.meta.url).href;
const deps: Record<string, Dep> = {};

function getSubmoduleNameFromUrl(url: string) {
  return url.replace(root, "").split("/")[0];
}

async function check(
  submod: string,
  state: DepState,
  paths: string[] = ["mod.ts"],
): Promise<Dep> {
  const deps = new Set<string>();
  for (const path of paths) {
    const entrypoint = new URL(`../${submod}/${path}`, import.meta.url).href;
    const graph = await createGraph(entrypoint);

    for (
      const dep of new Set(getSubmoduleDepsFromSpecifier(graph, entrypoint))
    ) {
      deps.add(dep);
    }
  }
  deps.delete(submod);
  deps.delete("types.d.ts");
  return { name: submod, set: deps, state };
}

/** Returns submodule dependencies */
function getSubmoduleDepsFromSpecifier(
  graph: ModuleGraphJson,
  specifier: string,
  seen: Set<string> = new Set(),
): Set<string> {
  const { dependencies } = graph.modules.find((item: ModuleJson) =>
    item.specifier === specifier
  )!;
  const deps = new Set([getSubmoduleNameFromUrl(specifier)]);
  seen.add(specifier);
  if (dependencies) {
    for (const { code, type } of dependencies) {
      const specifier = code?.specifier ?? type?.specifier!;
      if (seen.has(specifier)) {
        continue;
      }
      const res = getSubmoduleDepsFromSpecifier(
        graph,
        specifier,
        seen,
      )!;
      for (const dep of res) {
        deps.add(dep);
      }
    }
  }
  return deps;
}

deps["archive"] = await check("archive", "not ready");
deps["assert"] = await check("assert", "ready");
deps["async"] = await check("async", "ready");
deps["bytes"] = await check("bytes", "ready");
deps["collections"] = await check("collections", "ready");
deps["console"] = await check("console", "not ready");
deps["crypto"] = await check("crypto", "needs clean up");
deps["csv"] = await check("csv", "ready");
deps["data_structures"] = await check("data_structures", "not ready");
deps["datetime"] = await check("datetime", "deprecated");
deps["dotenv"] = await check("dotenv", "not ready");
deps["encoding"] = await check("encoding", "needs clean up", [
  "ascii85.ts",
  "base32.ts",
  "base58.ts",
  "base64.ts",
  "base64url.ts",
  "binary.ts",
  "hex.ts",
  "varint.ts",
]);
deps["flags"] = await check("flags", "not ready");
deps["fmt"] = await check("fmt", "ready", [
  "bytes.ts",
  "colors.ts",
  "duration.ts",
  "printf.ts",
]);
deps["front_matter"] = await check("front_matter", "needs clean up");
deps["fs"] = await check("fs", "ready");
deps["html"] = await check("html", "not ready");
deps["http"] = await check("http", "needs clean up");
deps["io"] = await check("io", "deprecated");
deps["json"] = await check("json", "ready");
deps["jsonc"] = await check("jsonc", "ready");
deps["log"] = await check("log", "not ready");
deps["media_types"] = await check("media_types", "ready");
deps["msgpack"] = await check("msgpack", "not ready");
deps["path"] = await check("path", "needs clean up");
deps["permissions"] = await check("permissions", "deprecated");
deps["regexp"] = await check("regexp", "not ready");
deps["semver"] = await check("semver", "not ready");
deps["signal"] = await check("signal", "deprecated");
deps["streams"] = await check("streams", "needs clean up");
deps["testing"] = await check("testing", "ready", [
  "bdd.ts",
  "mock.ts",
  "snapshot.ts",
  "time.ts",
  "types.ts",
]);
deps["toml"] = await check("toml", "ready");
deps["ulid"] = await check("ulid", "not ready");
deps["url"] = await check("url", "not ready");
deps["uuid"] = await check("uuid", "ready");
deps["yaml"] = await check("yaml", "ready");

/** Checks circular deps between sub modules */
function checkCircularDeps(
  submod: string,
  ancestors: string[] = [],
  seen: Set<string> = new Set(),
): string[] | undefined {
  const currentDeps = [...ancestors, submod];
  if (ancestors.includes(submod)) {
    return currentDeps;
  }
  const d = deps[submod];
  if (!d) {
    return;
  }
  for (const mod of d.set) {
    const res = checkCircularDeps(mod, currentDeps, seen);
    if (res) {
      return res;
    }
  }
}

/** Formats label for diagram */
function formatLabel(mod: string) {
  return '"' + mod.replace(/_/g, "_\\n") + '"';
}

/** Returns node style (in DOT language) for each state */
function stateToNodeStyle(state: DepState) {
  switch (state) {
    case "ready":
      return "[shape=doublecircle fixedsize=1 height=1.1]";
    case "not ready":
      return "[shape=box style=filled, fillcolor=pink]";
    case "needs clean up":
      return "[shape=circle fixedsize=1 height=1.1 style=filled, fillcolor=yellow]";
    case "deprecated":
      return "[shape=septagon style=filled, fillcolor=gray]";
  }
}

if (Deno.args.includes("--graph")) {
  console.log("digraph std_deps {");
  for (const mod of Object.keys(deps)) {
    const info = deps[mod];
    console.log(`  ${formatLabel(mod)} ${stateToNodeStyle(info.state)};`);
    for (const dep of deps[mod].set) {
      console.log(`  ${formatLabel(mod)} -> ${dep};`);
    }
  }
  console.log("}");
} else {
  console.log(`${Object.keys(deps).length} submodules checked.`);
  for (const mod of Object.keys(deps)) {
    const res = checkCircularDeps(mod);
    if (res) {
      console.log(`Circular dependencies found: ${res.join(" -> ")}`);
      Deno.exit(1);
    }
  }
  console.log("No circular dependencies found.");
}
