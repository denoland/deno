// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// This code has been inspired by https://github.com/bevry/domain-browser/commit/8bce7f4a093966ca850da75b024239ad5d0b33c6

import { EventEmitter } from "node:events";

export function create() {
  return new Domain();
}
export class Domain extends EventEmitter {
  constructor() {
    super();
  }
}
export default {
  create,
  Domain,
};
