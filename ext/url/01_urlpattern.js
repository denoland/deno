// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="./lib.deno_url.d.ts" />

import { primordials } from "ext:core/mod.js";
import { URLPattern } from "ext:core/ops";
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

import * as webidl from "ext:deno_webidl/00_webidl.js";
import { createFilteredInspectProxy } from "ext:deno_console/01_console.js";

webidl.configureInterface(URLPattern);
const URLPatternPrototype = URLPattern.prototype;

ObjectDefineProperty(
  URLPatternPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    value(inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(URLPatternPrototype, this),
          keys: [
            "protocol",
            "username",
            "password",
            "hostname",
            "port",
            "pathname",
            "search",
            "hash",
            "hasRegExpGroups",
          ],
        }),
        inspectOptions,
      );
    },
  },
);

export { URLPattern };
