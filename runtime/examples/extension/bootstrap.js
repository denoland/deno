// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { op_hello } from "ext:core/ops";
function hello() {
  op_hello("world");
}

globalThis.Extension = { hello };
