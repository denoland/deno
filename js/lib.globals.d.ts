// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This file contains the default TypeScript libraries for the runtime

/// <reference no-default-lib="true"/>

/// <reference lib="esnext" />

import "gen/js/globals";

interface Window {
  // TODO(ry) These shouldn't be global.
  mainSource: string;
  setMainSourceMap(sm: string): void;
}

// TODO(ry) These shouldn't be global.
declare let mainSource: string;
declare function setMainSourceMap(sm: string): void;
