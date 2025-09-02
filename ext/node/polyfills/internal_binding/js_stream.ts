// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and other Node contributors.

// This module provides a JavaScript stream wrapper that mimics the Node.js
// JSStream C++ binding for stream wrapping functionality.

"use strict";

// No primordials needed for this module

import { ownerSymbol } from "ext:deno_node/internal/async_hooks.ts";
import { providerType } from "ext:deno_node/internal_binding/async_wrap.ts";
import type {
  ShutdownWrap,
  WriteWrap,
} from "ext:deno_node/internal_binding/stream_wrap.ts";
import { HandleWrap } from "ext:deno_node/internal_binding/handle_wrap.ts";

// Simulate Node.js's JSStream C++ binding
export class JSStream extends HandleWrap {
  [ownerSymbol]: unknown;

  close?: (cb?: () => void) => void;
  isClosing?: () => boolean;
  onreadstart?: () => number;
  onreadstop?: () => number;
  onshutdown?: (req: ShutdownWrap<HandleWrap>) => number;
  onwrite?: (req: WriteWrap<HandleWrap>, bufs: Uint8Array[]) => number;

  constructor() {
    super(providerType.JSSTREAM);
  }

  readBuffer(_chunk: Uint8Array): void {
    // This simulates the C++ readBuffer functionality
    // In Node.js, this would trigger the onread callback
    // We'll implement this as needed
  }

  emitEOF(): void {
    // This simulates EOF emission from C++ side
    // Implementation will depend on the owner socket
  }

  finishWrite(req: WriteWrap<HandleWrap> | null, errCode: number): void {
    // Simulate the C++ finishWrite functionality
    if (req && req.oncomplete) {
      req.oncomplete(errCode);
    }
  }

  finishShutdown(req: ShutdownWrap<HandleWrap> | null, errCode: number): void {
    // Simulate the C++ finishShutdown functionality
    if (req && req.oncomplete) {
      req.oncomplete(errCode);
    }
  }
}
