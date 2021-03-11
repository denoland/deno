// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// The only purpose of this file is to set up "globalThis.__bootstrap" namespace,
// that is used by scripts in this directory to reference exports between
// the files.

// This namespace is removed during runtime bootstrapping process.
"use strict";

globalThis.__bootstrap = globalThis.__bootstrap || {};
