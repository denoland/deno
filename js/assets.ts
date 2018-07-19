// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

// This file is formatted as it is because we are using the fact that Parcel
// statically evaluates fs.readFileSync.
import { readFileSync } from "fs";

// tslint:disable:max-line-length
// prettier-ignore
export const assetSourceCode: { [key: string]: string } = {
  "deno.d.ts": readFileSync(__dirname + "/deno.d.ts", "utf8"),
  "lib.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.d.ts", "utf8"),
  //"lib.dom.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.dom.d.ts", "utf8"),
  "lib.dom.iterable.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.dom.iterable.d.ts", "utf8"),
  "lib.es2015.collection.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.collection.d.ts", "utf8"),
  "lib.es2015.core.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.core.d.ts", "utf8"),
  //"lib.es2015.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.d.ts", "utf8"),
  "lib.es2015.generator.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.generator.d.ts", "utf8"),
  "lib.es2015.iterable.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.iterable.d.ts", "utf8"),
  "lib.es2015.promise.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.promise.d.ts", "utf8"),
  "lib.es2015.proxy.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.proxy.d.ts", "utf8"),
  "lib.es2015.reflect.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.reflect.d.ts", "utf8"),
  "lib.es2015.symbol.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.symbol.d.ts", "utf8"),
  "lib.es2015.symbol.wellknown.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2015.symbol.wellknown.d.ts", "utf8"),
  "lib.es2016.array.include.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2016.array.include.d.ts", "utf8"),
  //"lib.es2016.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2016.d.ts", "utf8"),
  //"lib.es2016.full.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2016.full.d.ts", "utf8"),
  //"lib.es2017.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.d.ts", "utf8"),
  //"lib.es2017.full.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.full.d.ts", "utf8"),
  "lib.es2017.intl.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.intl.d.ts", "utf8"),
  "lib.es2017.object.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.object.d.ts", "utf8"),
  "lib.es2017.sharedmemory.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.sharedmemory.d.ts", "utf8"),
  "lib.es2017.string.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.string.d.ts", "utf8"),
  "lib.es2017.typedarrays.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2017.typedarrays.d.ts", "utf8"),
  "lib.es2018.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2018.d.ts", "utf8"),
  //"lib.es2018.full.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2018.full.d.ts", "utf8"),
  "lib.es2018.promise.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2018.promise.d.ts", "utf8"),
  "lib.es2018.regexp.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es2018.regexp.d.ts", "utf8"),
  //"lib.es5.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es5.d.ts", "utf8"),
  //"lib.es6.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.es6.d.ts", "utf8"),
  "lib.esnext.array.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.esnext.array.d.ts", "utf8"),
  "lib.esnext.asynciterable.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.esnext.asynciterable.d.ts", "utf8"),
  "lib.esnext.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.esnext.d.ts", "utf8"),
  //"lib.esnext.full.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.esnext.full.d.ts", "utf8"),
  //"lib.scripthost.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.scripthost.d.ts", "utf8"),
  //"lib.webworker.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/lib.webworker.d.ts", "utf8"),
  //"protocol.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/protocol.d.ts", "utf8"),
  //"tsserverlibrary.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/tsserverlibrary.d.ts", "utf8"),
  "typescript.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/typescript.d.ts", "utf8"),
  //"typescriptServices.d.ts": readFileSync(__dirname + "/../third_party/node_modules/typescript/lib/typescriptServices.d.ts", "utf8"),
};
