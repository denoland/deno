// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare class Window extends EventTarget {
  new(): Window;
  readonly window: Window & typeof globalThis;
  readonly self: Window & typeof globalThis;
  onload: ((this: Window, ev: Event) => any) | null;
  onunload: ((this: Window, ev: Event) => any) | null;
  close: () => void;
  readonly closed: boolean;
  alert: (message?: string) => void;
  confirm: (message?: string) => boolean;
  prompt: (message?: string, defaultValue?: string) => string | null;
  Deno: typeof Deno;
}

declare var window: Window & typeof globalThis;
declare var self: Window & typeof globalThis;
declare var onload: ((this: Window, ev: Event) => any) | null;
declare var onunload: ((this: Window, ev: Event) => any) | null;

/**
 * Shows the given message and waits for the enter key pressed.
 * If the stdin is not interactive, it does nothing.
 * @param message
 */
declare function alert(message?: string): void;

/**
 * Shows the given message and waits for the answer. Returns the user's answer as boolean.
 * Only `y` and `Y` are considered as true.
 * If the stdin is not interactive, it returns false.
 * @param message
 */
declare function confirm(message?: string): boolean;

/**
 * Shows the given message and waits for the user's input. Returns the user's input as string.
 * If the default value is given and the user inputs the empty string, then it returns the given
 * default value.
 * If the default value is not given and the user inputs the empty string, it returns null.
 * If the stdin is not interactive, it returns null.
 * @param message
 * @param defaultValue
 */
declare function prompt(message?: string, defaultValue?: string): string | null;
