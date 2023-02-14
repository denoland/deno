// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// Lazily initializes the actual stdio objects.
// This trick is necessary for avoiding circular dependencies between
// stream and process modules.
export const stdio = {};
