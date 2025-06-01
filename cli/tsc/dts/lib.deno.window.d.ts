// Copyright 2018-2025 the Deno authors. MIT license.

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

/**
 * The window variable was removed in Deno 2. This declaration should be
 * removed at some point, but we're leaving it in out of caution.
 * @ignore
 * @category Platform
 */
declare var window: Window & typeof globalThis;
/** @category Platform */
declare var self: Window & typeof globalThis;
/** @category Platform */
declare var closed: boolean;

/**
 * Exits the current Deno process.
 *
 * This function terminates the process by signaling the runtime to exit.
 * Similar to exit(0) in posix. Its behavior is similar to the `window.close()`
 * method in the browser, but specific to the Deno runtime.
 *
 * Note: Use this function cautiously, as it will stop the execution of the
 * entire Deno program immediately.
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/close
 *
 * @example
 * ```ts
 * console.log("About to close the Deno process.");
 * close(); // The process will terminate here.
 * console.log("This will not be logged."); // This line will never execute.
 * ```
 *
 * @category Platform
 */
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
/**
 * Deno's `localStorage` API provides a way to store key-value pairs in a
 * web-like environment, similar to the Web Storage API found in browsers.
 * It allows developers to persist data across sessions in a Deno application.
 * This API is particularly useful for applications that require a simple
 * and effective way to store data locally.
 *
 * - Key-Value Storage: Stores data as key-value pairs.
 * - Persistent: Data is retained even after the application is closed.
 * - Synchronous API: Operations are performed synchronously.
 *
 * `localStorage` is similar to {@linkcode sessionStorage}, and shares the same
 * API methods, visible in the {@linkcode Storage} type.
 *
 * When using the `--location` flag, the origin for the location is used to
 * uniquely store the data. That means a location of http://example.com/a.ts
 * and http://example.com/b.ts and http://example.com:80/ would all share the
 * same storage, but https://example.com/ would be different.
 *
 * For more information, see the reference guide for
 * [Web Storage](https://docs.deno.com/runtime/reference/web_platform_apis/#web-storage)
 * and using
 * [the `--location` flag](https://docs.deno.com/runtime/reference/web_platform_apis/#location-flag).
 *
 * @example
 * ```ts
 * // Set a value in localStorage
 * localStorage.setItem("key", "value");
 *
 * // Get a value from localStorage
 * const value = localStorage.getItem("key");
 * console.log(value); // Output: "value"
 *
 * // Remove a value from localStorage
 * localStorage.removeItem("key");
 *
 * // Clear all values from localStorage
 * localStorage.clear();
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage
 * @category Storage */
declare var localStorage: Storage;

/**
 * Deno's `sessionStorage` API operates similarly to the {@linkcode localStorage} API,
 * but it is intended for storing data temporarily for the duration of a session.
 * Data stored in sessionStorage is cleared when the application session or
 * process ends. This makes it suitable for temporary data that you do not need
 * to persist across user sessions.
 *
 * - Key-Value Storage: Stores data as key-value pairs.
 * - Session-Based: Data is only available for the duration of the page session.
 * - Synchronous API: Operations are performed synchronously.
 *
 * `sessionStorage` is similar to {@linkcode localStorage}, and shares the same API
 * methods, visible in the {@linkcode Storage} type.
 *
 * For more information, see the reference guide for
 * [Web Storage](https://docs.deno.com/runtime/reference/web_platform_apis/#web-storage)
 *
 * @example
 * ```ts
 * // Set a value in sessionStorage
 * sessionStorage.setItem("key", "value");
 *
 * // Get a value from sessionStorage
 * const value = sessionStorage.getItem("key");
 * console.log(value); // Output: "value"
 *
 * // Remove a value from sessionStorage
 * sessionStorage.removeItem("key");
 *
 * // Clear all the values from sessionStorage
 * sessionStorage.clear();
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/sessionStorage
 * @category Storage
 */
declare var sessionStorage: Storage;
/** @category Cache */
/**
 * Provides access to the Cache API. Returns a CacheStorage object, which enables storing, retrieving, and managing request/response pairs in a cache.
 *
 * @example
 * ```ts
 * // Open (or create) a cache
 * const cache = await caches.open('v1');
 *
 * // Store a response
 * await cache.put('/api/data', new Response('Hello World'));
 *
 * // Retrieve from cache with fallback
 * const response = await caches.match('/api/data') || await fetch('/api/data');
 *
 * // Delete specific cache
 * await caches.delete('v1');
 *
 * // List all cache names
 * const cacheNames = await caches.keys();
 *
 * // Cache-first strategy
 * async function fetchWithCache(request) {
 *   const cached = await caches.match(request);
 *   if (cached) return cached;
 *
 *   const response = await fetch(request);
 *   const cache = await caches.open('v1');
 *   await cache.put(request, response.clone());
 *   return response;
 * }
 * ```
 *
 * @see  https://developer.mozilla.org/en-US/docs/Web/API/Window/caches
 */
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
 * @example
 * ```ts
 * // Displays the message "Acknowledge me! [Enter]" and waits for the enter key to be pressed before continuing.
 * alert("Acknowledge me!");
 * ```
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/alert
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
 * @example
 * ```ts
 * const shouldProceed = confirm("Do you want to proceed?");
 *
 * // If the user presses 'y' or 'Y', the result will be true
 * // If the user presses 'n' or 'N', the result will be false
 * console.log("Should proceed?", shouldProceed);
 * ```
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/confirm
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
 * @example
 * ```ts
 * const pet = prompt("Cats or dogs?", "It's fine to love both!");
 *
 * // Displays the user's input or the default value of "It's fine to love both!"
 * console.log("Best pet:", pet);
 * ```
 * @see https://developer.mozilla.org/en-US/docs/Web/API/Window/prompt
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
