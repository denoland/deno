// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="deno.webstorage" />
/// <reference lib="esnext" />
/// <reference lib="deno.cache" />

/** @category Platform */
interface WindowEventMap {
  "error": ErrorEvent;
  "unhandledrejection": PromiseRejectionEvent;
  "rejectionhandled": PromiseRejectionEvent;
}

/** @category Platform */
interface Window extends EventTarget {
  readonly window: Window & typeof globalThis;
  readonly self: Window & typeof globalThis;
  onerror: ((this: Window, ev: ErrorEvent) => any) | null;
  onload: ((this: Window, ev: Event) => any) | null;
  onbeforeunload: ((this: Window, ev: Event) => any) | null;
  onunload: ((this: Window, ev: Event) => any) | null;
  onunhandledrejection:
    | ((this: Window, ev: PromiseRejectionEvent) => any)
    | null;
  onrejectionhandled:
    | ((this: Window, ev: PromiseRejectionEvent) => any)
    | null;
  close: () => void;
  readonly closed: boolean;
  alert: (message?: string) => void;
  confirm: (message?: string) => boolean;
  prompt: (message?: string, defaultValue?: string) => string | null;
  Deno: typeof Deno;
  Navigator: typeof Navigator;
  navigator: Navigator;
  Location: typeof Location;
  location: Location;
  localStorage: Storage;
  sessionStorage: Storage;
  caches: CacheStorage;
  name: string;

  addEventListener<K extends keyof WindowEventMap>(
    type: K,
    listener: (
      this: Window,
      ev: WindowEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof WindowEventMap>(
    type: K,
    listener: (
      this: Window,
      ev: WindowEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** @category Platform */
declare var Window: {
  readonly prototype: Window;
  new (): never;
};

/** @category Platform */
declare var window: Window & typeof globalThis;
/** @category Platform */
declare var self: Window & typeof globalThis;
/** @category Platform */
declare var closed: boolean;
/** @category Platform */
declare function close(): void;
/** @category Events */
declare var onerror: ((this: Window, ev: ErrorEvent) => any) | null;
/** @category Events */
declare var onload: ((this: Window, ev: Event) => any) | null;
/** @category Events */
declare var onbeforeunload: ((this: Window, ev: Event) => any) | null;
/** @category Events */
declare var onunload: ((this: Window, ev: Event) => any) | null;
/** @category Events */
declare var onunhandledrejection:
  | ((this: Window, ev: PromiseRejectionEvent) => any)
  | null;
/** @category Storage */
declare var localStorage: Storage;
/** @category Storage */
declare var sessionStorage: Storage;
/** @category Cache */
declare var caches: CacheStorage;

/** @category Platform */
interface Navigator {
  readonly gpu: GPU;
  readonly hardwareConcurrency: number;
  readonly userAgent: string;
  readonly language: string;
  readonly languages: string[];
}

/** @category Platform */
declare var Navigator: {
  readonly prototype: Navigator;
  new (): never;
};

/** @category Platform */
declare var navigator: Navigator;

/**
 * Shows the given message and waits for the enter key pressed.
 *
 * If the stdin is not interactive, it does nothing.
 *
 * @category Platform
 *
 * @param message
 */
declare function alert(message?: string): void;

/**
 * Shows the given message and waits for the answer. Returns the user's answer as boolean.
 *
 * Only `y` and `Y` are considered as true.
 *
 * If the stdin is not interactive, it returns false.
 *
 * @category Platform
 *
 * @param message
 */
declare function confirm(message?: string): boolean;

/**
 * Shows the given message and waits for the user's input. Returns the user's input as string.
 *
 * If the default value is given and the user inputs the empty string, then it returns the given
 * default value.
 *
 * If the default value is not given and the user inputs the empty string, it returns the empty
 * string.
 *
 * If the stdin is not interactive, it returns null.
 *
 * @category Platform
 *
 * @param message
 * @param defaultValue
 */
declare function prompt(message?: string, defaultValue?: string): string | null;

/** Registers an event listener in the global scope, which will be called
 * synchronously whenever the event `type` is dispatched.
 *
 * ```ts
 * addEventListener('unload', () => { console.log('All finished!'); });
 * ...
 * dispatchEvent(new Event('unload'));
 * ```
 *
 * @category Events
 */
declare function addEventListener<
  K extends keyof WindowEventMap,
>(
  type: K,
  listener: (this: Window, ev: WindowEventMap[K]) => any,
  options?: boolean | AddEventListenerOptions,
): void;
/** @category Events */
declare function addEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions,
): void;

/** Remove a previously registered event listener from the global scope
 *
 * ```ts
 * const listener = () => { console.log('hello'); };
 * addEventListener('load', listener);
 * removeEventListener('load', listener);
 * ```
 *
 * @category Events
 */
declare function removeEventListener<
  K extends keyof WindowEventMap,
>(
  type: K,
  listener: (this: Window, ev: WindowEventMap[K]) => any,
  options?: boolean | EventListenerOptions,
): void;
/** @category Events */
declare function removeEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | EventListenerOptions,
): void;

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** The location (URL) of the object it is linked to. Changes done on it are
 * reflected on the object it relates to. Accessible via
 * `globalThis.location`.
 *
 * @category Platform
 */
interface Location {
  /** Returns a DOMStringList object listing the origins of the ancestor
   * browsing contexts, from the parent browsing context to the top-level
   * browsing context.
   *
   * Always empty in Deno. */
  readonly ancestorOrigins: DOMStringList;
  /** Returns the Location object's URL's fragment (includes leading "#" if
   * non-empty).
   *
   * Cannot be set in Deno. */
  hash: string;
  /** Returns the Location object's URL's host and port (if different from the
   * default port for the scheme).
   *
   * Cannot be set in Deno. */
  host: string;
  /** Returns the Location object's URL's host.
   *
   * Cannot be set in Deno. */
  hostname: string;
  /** Returns the Location object's URL.
   *
   * Cannot be set in Deno. */
  href: string;
  toString(): string;
  /** Returns the Location object's URL's origin. */
  readonly origin: string;
  /** Returns the Location object's URL's path.
   *
   * Cannot be set in Deno. */
  pathname: string;
  /** Returns the Location object's URL's port.
   *
   * Cannot be set in Deno. */
  port: string;
  /** Returns the Location object's URL's scheme.
   *
   * Cannot be set in Deno. */
  protocol: string;
  /** Returns the Location object's URL's query (includes leading "?" if
   * non-empty).
   *
   * Cannot be set in Deno. */
  search: string;
  /** Navigates to the given URL.
   *
   * Cannot be set in Deno. */
  assign(url: string): void;
  /** Reloads the current page.
   *
   * Disabled in Deno. */
  reload(): void;
  /** @deprecated */
  reload(forcedReload: boolean): void;
  /** Removes the current page from the session history and navigates to the
   * given URL.
   *
   * Disabled in Deno. */
  replace(url: string): void;
}

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** The location (URL) of the object it is linked to. Changes done on it are
 * reflected on the object it relates to. Accessible via
 * `globalThis.location`.
 *
 * @category Platform
 */
declare var Location: {
  readonly prototype: Location;
  new (): never;
};

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** @category Platform */
declare var location: Location;

/** @category Platform */
declare var name: string;
