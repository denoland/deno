// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { Console } from "./console.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync } from "./dispatch_json.ts";
import { TextEncoder } from "./text_encoding.ts";
import { assert, commonPath, humanFileSize } from "./util.ts";
import { writeFileSync } from "./write_file.ts";

declare global {
  const console: Console;
}

const BUNDLE_LOADER = "bundle_loader.js";

const encoder = new TextEncoder();

let bundleLoader: string;

let rootExports: string[] | undefined;

/** Given a fileName and the data, emit the file to the file system. */
export function emitBundle(
  rootNames: string[],
  fileName: string | undefined,
  data: string,
  sourceFiles: readonly ts.SourceFile[]
): void {
  // if the fileName is set to an internal value, just noop
  if (fileName && fileName.startsWith("$deno$")) {
    return;
  }
  // This should never happen at the moment, but this code can't currently
  // support it
  assert(
    rootNames.length === 1,
    "Only single root modules supported for bundling."
  );
  if (!bundleLoader) {
    bundleLoader = sendSync(dispatch.OP_FETCH_ASSET, { name: BUNDLE_LOADER });
  }

  // when outputting to AMD and a single outfile, TypeScript makes up the module
  // specifiers which are used to define the modules, and doesn't expose them
  // publicly, so we have to try to replicate
  const sources = sourceFiles.map(sf => sf.fileName);
  const sharedPath = commonPath(sources);
  const rootName = rootNames[0].replace(sharedPath, "").replace(/\.\w+$/i, "");
  let instantiate: string;
  if (rootExports && rootExports.length) {
    instantiate = `const __rootExports = instantiate("${rootName}");\n`;
    for (const rootExport of rootExports) {
      if (rootExport === "default") {
        instantiate += `export default __rootExports["${rootExport}"];\n`;
      } else {
        instantiate += `export const ${rootExport} = __rootExports["${rootExport}"];\n`;
      }
    }
  } else {
    instantiate = `instantiate("${rootName}");\n`;
  }
  const bundle = `${bundleLoader}\n${data}\n${instantiate}`;
  if (fileName) {
    const encodedData = encoder.encode(bundle);
    console.warn(`Emitting bundle to "${fileName}"`);
    writeFileSync(fileName, encodedData);
    console.warn(`${humanFileSize(encodedData.length)} emitted.`);
  } else {
    console.log(bundle);
  }
}

/** Set the rootExports which will by the `emitBundle()` */
export function setRootExports(
  program: ts.Program,
  rootModules: string[]
): void {
  // get a reference to the type checker, this will let us find symbols from
  // the AST.
  const checker = program.getTypeChecker();
  assert(rootModules.length === 1);
  // get a reference to the main source file for the bundle
  const mainSourceFile = program.getSourceFile(rootModules[0]);
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
      sym =>
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
    .map(sym => sym.getName());
}
