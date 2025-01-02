// Copyright 2018-2025 the Deno authors. MIT license.
import { op_hello } from "ext:core/ops";
function hello() {
  op_hello("world");
}

globalThis.Extension = { hello };
