// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare interface Window extends EventTarget {
  readonly window: Window & typeof globalThis;
  readonly self: Window & typeof globalThis;
  onload: ((this: Window, ev: Event) => any) | null;
  onunload: ((this: Window, ev: Event) => any) | null;
  close: () => void;
  readonly closed: boolean;
  Deno: typeof Deno;
}

declare const window: Window & typeof globalThis;
declare const self: Window & typeof globalThis;
declare const onload: ((this: Window, ev: Event) => any) | null;
declare const onunload: ((this: Window, ev: Event) => any) | null;

/* eslint-enable @typescript-eslint/no-explicit-any */
