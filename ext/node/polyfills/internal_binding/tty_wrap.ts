// Copyright 2018-2025 the Deno authors. MIT license.

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
}
