// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// The only purpose of this file is to set up "window.__bootstrap" namespace,
// that is used by scripts in this directory to reference exports between
// the files.

// This namespace is removed during runtime bootstrapping process.

((window) => {
  window.__bootstrap = {};
})(this);
