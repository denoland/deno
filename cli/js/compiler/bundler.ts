// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { SYSTEM_LOADER } from "./bootstrap.ts";
import { commonPath, normalizeString, CHAR_FORWARD_SLASH } from "./util.ts";
import { assert } from "../util.ts";

let rootExports: string[] | undefined;

function normalizeUrl(rootName: string): string {
  const match = /^(\S+:\/{2,3})(.+)$/.exec(rootName);
  if (match) {
    const [, protocol, path] = match;
    return `${protocol}${normalizeString(
      path,
      false,
      "/",
      (code) => code === CHAR_FORWARD_SLASH
    )}`;
  } else {
    return rootName;
  }
}

export function buildBundle(
  rootName: string,
  data: string,
  sourceFiles: readonly ts.SourceFile[]
): string {
  // when outputting to AMD and a single outfile, TypeScript makes up the module
  // specifiers which are used to define the modules, and doesn't expose them
  // publicly, so we have to try to replicate
  const sources = sourceFiles.map((sf) => sf.fileName);
  const sharedPath = commonPath(sources);
  rootName = normalizeUrl(rootName)
    .replace(sharedPath, "")
    .replace(/\.\w+$/i, "");
  // If one of the modules requires support for top-level-await, TypeScript will
  // emit the execute function as an async function.  When this is the case we
  // need to bubble up the TLA to the instantiation, otherwise we instantiate
  // synchronously.
  const hasTla = data.match(/execute:\sasync\sfunction\s/);
  let instantiate: string;
  if (rootExports && rootExports.length) {
    instantiate = hasTla
      ? `const __exp = await __instantiateAsync("${rootName}");\n`
      : `const __exp = __instantiate("${rootName}");\n`;
    for (const rootExport of rootExports) {
      if (rootExport === "default") {
        instantiate += `export default __exp["${rootExport}"];\n`;
      } else {
        instantiate += `export const ${rootExport} = __exp["${rootExport}"];\n`;
      }
    }
  } else {
    instantiate = hasTla
      ? `await __instantiateAsync("${rootName}");\n`
      : `__instantiate("${rootName}");\n`;
  }
  return `${SYSTEM_LOADER}\n${data}\n${instantiate}`;
}

export function setRootExports(program: ts.Program, rootModule: string): void {
  // get a reference to the type checker, this will let us find symbols from
  // the AST.
  const checker = program.getTypeChecker();
  // get a reference to the main source file for the bundle
  const mainSourceFile = program.getSourceFile(rootModule);
  assert(mainSourceFile);
  // retrieve the internal TypeScript symbol for this AST node
  const mainSymbol = checker.getSymbolAtLocation(mainSourceFile);
  if (!mainSymbol) {
    return;
  }
  rootExports = checker
    .getExportsOfModule(mainSymbol)
    // .getExportsOfModule includes type only symbols which are exported from
    // the module, so we need to try to filter those out.  While not critical
    // someone looking at the bundle would think there is runtime code behind
    // that when there isn't.  There appears to be no clean way of figuring that
    // out, so inspecting SymbolFlags that might be present that are type only
    .filter(
      (sym) =>
        sym.flags & ts.SymbolFlags.Class ||
        !(
          sym.flags & ts.SymbolFlags.Interface ||
          sym.flags & ts.SymbolFlags.TypeLiteral ||
          sym.flags & ts.SymbolFlags.Signature ||
          sym.flags & ts.SymbolFlags.TypeParameter ||
          sym.flags & ts.SymbolFlags.TypeAlias ||
          sym.flags & ts.SymbolFlags.Type ||
          sym.flags & ts.SymbolFlags.Namespace ||
          sym.flags & ts.SymbolFlags.InterfaceExcludes ||
          sym.flags & ts.SymbolFlags.TypeParameterExcludes ||
          sym.flags & ts.SymbolFlags.TypeAliasExcludes
        )
    )
    .map((sym) => sym.getName());
}
