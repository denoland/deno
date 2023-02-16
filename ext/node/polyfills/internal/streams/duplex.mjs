// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.
// deno-lint-ignore-file

import { Duplex } from "internal:deno_node/polyfills/_stream.mjs";
const { from, fromWeb, toWeb } = Duplex;

export default Duplex;
export { from, fromWeb, toWeb };
