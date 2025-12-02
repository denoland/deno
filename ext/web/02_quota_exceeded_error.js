// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="./internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

import { primordials } from "ext:core/mod.js";
const {
  Symbol,
} = primordials;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";

const _name = Symbol("name");
const _message = Symbol("message");
const _code = Symbol("code");

class QuotaExceededError extends DOMException {
  [_name] = "QuotaExceededError";
  [_message];
  [_code] = 22;

  constructor(message = "", options = { __proto__: null }) {
    super(message, "QuotaExceededError", options);
  }
}

webidl.configureInterface(QuotaExceededError);

export { QuotaExceededError };
