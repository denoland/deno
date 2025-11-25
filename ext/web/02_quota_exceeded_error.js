// Copyright 2018-2025 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/internal.d.ts" />
/// <reference path="../../cli/tsc/dts/lib.deno_web.d.ts" />

import { primordials } from "ext:core/mod.js";
const {
  ObjectDefineProperty,
  Symbol,
} = primordials;
import * as webidl from "ext:deno_webidl/00_webidl.js";
import { DOMException } from "ext:deno_web/01_dom_exception.js";

const _name = Symbol("[[name]]");
const _message = Symbol("[[message]]");
const _code = Symbol("[[code]]");

class QuotaExceededError extends DOMException {
  constructor(message = "", options = {}) {
    super(message, "QuotaExceededError");
    this[webidl.brand] = webidl.brand;
    this[_name] = "QuotaExceededError";
    this[_message] = message;
    this[_code] = 22;
  }

  get name() {
    return this[_name];
  }

  get message() {
    return this[_message];
  }

  get code() {
    return this[_code];
  }
}

webidl.configureInterface(QuotaExceededError);

ObjectDefineProperty(globalThis, "QuotaExceededError", {
  value: QuotaExceededError,
  writable: true,
  enumerable: false,
  configurable: true,
});

export { QuotaExceededError };
