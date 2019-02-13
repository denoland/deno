// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// tslint:disable-next-line:no-reference
/// <reference path="./plugins.d.ts" />

// There is a rollup plugin that will inline any module ending with `!string`
// tslint:disable:max-line-length

// Generated default library
import libDts from "gen/lib/lib.deno_runtime.d.ts!string";

// Static libraries
import libEs2015Dts from "/third_party/node_modules/typescript/lib/lib.es2015.d.ts!string";
import libEs2015CollectionDts from "/third_party/node_modules/typescript/lib/lib.es2015.collection.d.ts!string";
import libEs2015CoreDts from "/third_party/node_modules/typescript/lib/lib.es2015.core.d.ts!string";
import libEs2015GeneratorDts from "/third_party/node_modules/typescript/lib/lib.es2015.generator.d.ts!string";
import libEs2015IterableDts from "/third_party/node_modules/typescript/lib/lib.es2015.iterable.d.ts!string";
import libEs2015PromiseDts from "/third_party/node_modules/typescript/lib/lib.es2015.promise.d.ts!string";
import libEs2015ProxyDts from "/third_party/node_modules/typescript/lib/lib.es2015.proxy.d.ts!string";
import libEs2015ReflectDts from "/third_party/node_modules/typescript/lib/lib.es2015.reflect.d.ts!string";
import libEs2015SymbolDts from "/third_party/node_modules/typescript/lib/lib.es2015.symbol.d.ts!string";
import libEs2015SymbolWellknownDts from "/third_party/node_modules/typescript/lib/lib.es2015.symbol.wellknown.d.ts!string";
import libEs2016Dts from "/third_party/node_modules/typescript/lib/lib.es2016.d.ts!string";
import libEs2016ArrayIncludeDts from "/third_party/node_modules/typescript/lib/lib.es2016.array.include.d.ts!string";
import libEs2017Dts from "/third_party/node_modules/typescript/lib/lib.es2017.d.ts!string";
import libEs2017IntlDts from "/third_party/node_modules/typescript/lib/lib.es2017.intl.d.ts!string";
import libEs2017ObjectDts from "/third_party/node_modules/typescript/lib/lib.es2017.object.d.ts!string";
import libEs2017SharedmemoryDts from "/third_party/node_modules/typescript/lib/lib.es2017.sharedmemory.d.ts!string";
import libEs2017StringDts from "/third_party/node_modules/typescript/lib/lib.es2017.string.d.ts!string";
import libEs2017TypedarraysDts from "/third_party/node_modules/typescript/lib/lib.es2017.typedarrays.d.ts!string";
import libEs2018Dts from "/third_party/node_modules/typescript/lib/lib.es2018.d.ts!string";
import libEs2018IntlDts from "/third_party/node_modules/typescript/lib/lib.es2018.intl.d.ts!string";
import libEs2018PromiseDts from "/third_party/node_modules/typescript/lib/lib.es2018.promise.d.ts!string";
import libEs2018RegexpDts from "/third_party/node_modules/typescript/lib/lib.es2018.regexp.d.ts!string";
import libEs5Dts from "/third_party/node_modules/typescript/lib/lib.es5.d.ts!string";
import libEsnextArrayDts from "/third_party/node_modules/typescript/lib/lib.esnext.array.d.ts!string";
import libEsnextAsynciterablesDts from "/third_party/node_modules/typescript/lib/lib.esnext.asynciterable.d.ts!string";
import libEsnextBigintDts from "/third_party/node_modules/typescript/lib/lib.esnext.bigint.d.ts!string";
import libEsnextDts from "/third_party/node_modules/typescript/lib/lib.esnext.d.ts!string";
import libEsnextIntlDts from "/third_party/node_modules/typescript/lib/lib.esnext.intl.d.ts!string";
import libEsnextSymbolDts from "/third_party/node_modules/typescript/lib/lib.esnext.symbol.d.ts!string";

// Static definitions
import typescriptDts from "/third_party/node_modules/typescript/lib/typescript.d.ts!string";
// tslint:enable:max-line-length

// @internal
export const assetSourceCode: { [key: string]: string } = {
  // Generated library
  "lib.deno_runtime.d.ts": libDts,

  // Static libraries
  "lib.es2015.collection.d.ts": libEs2015CollectionDts,
  "lib.es2015.core.d.ts": libEs2015CoreDts,
  "lib.es2015.d.ts": libEs2015Dts,
  "lib.es2015.generator.d.ts": libEs2015GeneratorDts,
  "lib.es2015.iterable.d.ts": libEs2015IterableDts,
  "lib.es2015.promise.d.ts": libEs2015PromiseDts,
  "lib.es2015.proxy.d.ts": libEs2015ProxyDts,
  "lib.es2015.reflect.d.ts": libEs2015ReflectDts,
  "lib.es2015.symbol.d.ts": libEs2015SymbolDts,
  "lib.es2015.symbol.wellknown.d.ts": libEs2015SymbolWellknownDts,
  "lib.es2016.array.include.d.ts": libEs2016ArrayIncludeDts,
  "lib.es2016.d.ts": libEs2016Dts,
  "lib.es2017.d.ts": libEs2017Dts,
  "lib.es2017.intl.d.ts": libEs2017IntlDts,
  "lib.es2017.object.d.ts": libEs2017ObjectDts,
  "lib.es2017.sharedmemory.d.ts": libEs2017SharedmemoryDts,
  "lib.es2017.string.d.ts": libEs2017StringDts,
  "lib.es2017.typedarrays.d.ts": libEs2017TypedarraysDts,
  "lib.es2018.d.ts": libEs2018Dts,
  "lib.es2018.intl.d.ts": libEs2018IntlDts,
  "lib.es2018.promise.d.ts": libEs2018PromiseDts,
  "lib.es2018.regexp.d.ts": libEs2018RegexpDts,
  "lib.es5.d.ts": libEs5Dts,
  "lib.esnext.d.ts": libEsnextDts,
  "lib.esnext.array.d.ts": libEsnextArrayDts,
  "lib.esnext.asynciterable.d.ts": libEsnextAsynciterablesDts,
  "lib.esnext.bigint.d.ts": libEsnextBigintDts,
  "lib.esnext.intl.d.ts": libEsnextIntlDts,
  "lib.esnext.symbol.d.ts": libEsnextSymbolDts,

  // Static definitions
  "typescript.d.ts": typescriptDts
};
