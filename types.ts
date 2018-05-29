// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
export type TypedArray = Uint8Array | Float32Array | Int32Array;

export interface ModuleInfo {
  moduleName?: string;
  filename?: string;
  sourceCode?: string;
  outputCode?: string;
}
