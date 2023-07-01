// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

import { finished, pipeline } from "ext:deno_node/_stream.mjs";

export default {
  finished,
  pipeline,
};
export { finished, pipeline };
