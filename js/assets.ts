// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// tslint:disable-next-line:no-reference
/// <reference path="./plugins.d.ts" />

// There is a rollup plugin that will inline any module ending with `!string`
// tslint:disable:max-line-length

// Generated default library
import globalsDts from "gen/types/globals.d.ts!string";

// Static libraries
import libEsnextDts from "/third_party/node_modules/typescript/lib/lib.esnext.d.ts!string";

// Static definitions
import fetchTypesDts from "/js/fetch_types.d.ts!string";
import flatbuffersDts from "/third_party/node_modules/@types/flatbuffers/index.d.ts!string";
import textEncodingDts from "/third_party/node_modules/@types/text-encoding/index.d.ts!string";
import typescriptDts from "/third_party/node_modules/typescript/lib/typescript.d.ts!string";
// tslint:enable:max-line-length

// @internal
export const assetSourceCode: { [key: string]: string } = {
  // Generated library
  "globals.d.ts": globalsDts,

  // Static libraries
  "lib.esnext.d.ts": libEsnextDts,

  // Static definitions
  "fetch-types.d.ts": fetchTypesDts,
  "flatbuffers.d.ts": flatbuffersDts,
  "text-encoding.d.ts": textEncodingDts,
  "typescript.d.ts": typescriptDts
};
