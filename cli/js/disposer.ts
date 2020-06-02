// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Disposer wraps the dispose method for any disposable objects
export interface Disposer {
  dispose(): void;
}
