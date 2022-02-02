// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true"/>

interface Error {
  cause?: any;
}

interface ErrorInit {
  cause?: any;
}

interface ErrorConstructor {
  new (message?: string, init?: ErrorInit): Error;
  (message?: string, init?: ErrorInit): Error;
}
