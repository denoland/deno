// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { Duplex } from "ext:deno_node/_stream.mjs";
const { from, fromWeb, toWeb } = Duplex;

export default Duplex;
export { from, fromWeb, toWeb };
