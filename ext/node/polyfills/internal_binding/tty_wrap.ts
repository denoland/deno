// Copyright 2018-2026 the Deno authors. MIT license.

import { TTY } from "ext:core/ops";

// Mark TTY as a StreamBase handle, matching Node's StreamBase::AddMethods.
TTY.prototype.isStreamBase = true;

export { TTY };
