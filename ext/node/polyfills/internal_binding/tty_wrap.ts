// Copyright 2018-2026 the Deno authors. MIT license.

import {
  kStreamBaseField,
  LibuvStreamWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { providerType } from "ext:deno_node/internal_binding/async_wrap.ts";
import * as io from "ext:deno_io/12_io.js";

export class TTY extends LibuvStreamWrap {
  constructor(handle) {
    super(providerType.TTYWRAP, handle);
  }

  ref() {
    this[kStreamBaseField][io.REF]();
  }

  unref() {
    this[kStreamBaseField][io.UNREF]();
  }

  setRaw(flag, options) {
    const stream = this[kStreamBaseField];
    if (stream && typeof stream.setRaw === "function") {
      stream.setRaw(flag, options);
    }
  }

  getWindowSize(size) {
    try {
      const { columns, rows } = Deno.consoleSize();
      size[0] = columns;
      size[1] = rows;
      return 0;
    } catch {
      return -1;
    }
  }
}
