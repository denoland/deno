// Copyright 2018-2026 the Deno authors. MIT license.

// Test that napi_wrap finalizers run at shutdown even when the wrapped
// JS object is still reachable (not garbage collected). This matches
// Node.js behavior where weak reference cleanup happens during
// napi_env::DeleteMe().

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

// Create an object and wrap it with a native finalizer.
// Keep the reference alive (in global scope) so GC won't collect it.
const _leaked = lib.test_wrap_leak({});

// The process exits naturally here. The wrap finalizer should still
// be called during shutdown, printing the message.
