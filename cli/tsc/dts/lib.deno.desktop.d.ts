// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.window" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="deno.webstorage" />
/// <reference lib="esnext" />
/// <reference lib="deno.cache" />
/// <reference lib="es2022.intl" />

declare interface UIEventInit extends EventInit {
  detail?: number;
  view?: null;
}

declare class UIEvent extends Event {
  constructor(type: string, init?: UIEventInit);
  readonly detail: number;
  readonly view: null;
}

declare interface FocusEventInit extends UIEventInit {
  relatedTarget?: EventTarget | null;
}

declare class FocusEvent extends UIEvent {
  constructor(type: string, init?: FocusEventInit);
  readonly relatedTarget: EventTarget | null;
}

declare interface KeyboardEventInit extends UIEventInit {
  key?: string;
  code?: string;
  location?: number;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  repeat?: boolean;
  isComposing?: boolean;
}

declare class KeyboardEvent extends UIEvent {
  constructor(type: string, init?: KeyboardEventInit);
  readonly key: string;
  readonly code: string;
  readonly location: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  readonly repeat: boolean;
  readonly isComposing: boolean;
  getModifierState(key: string): boolean;
}

declare interface MouseEventInit extends UIEventInit {
  button?: number;
  clientX?: number;
  clientY?: number;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
}

declare class MouseEvent extends UIEvent {
  constructor(type: string, init?: MouseEventInit);
  readonly button: number;
  readonly clientX: number;
  readonly clientY: number;
  readonly screenX: number;
  readonly screenY: number;
  readonly ctrlKey: boolean;
  readonly shiftKey: boolean;
  readonly altKey: boolean;
  readonly metaKey: boolean;
  getModifierState(key: string): boolean;
}

declare interface WheelEventInit extends MouseEventInit {
  deltaX?: number;
  deltaY?: number;
  deltaZ?: number;
  deltaMode?: number;
}

declare class WheelEvent extends MouseEvent {
  constructor(type: string, init?: WheelEventInit);
  readonly deltaX: number;
  readonly deltaY: number;
  readonly deltaZ: number;
  readonly deltaMode: number;
}

declare type NotificationPermission = "default" | "denied" | "granted";
declare type NotificationDirection = "auto" | "ltr" | "rtl";

declare interface NotificationOptions {
  body?: string;
  data?: any;
  dir?: NotificationDirection;
  icon?: string;
  lang?: string;
  badge?: string;
  requireInteraction?: boolean;
  silent?: boolean | null;
  tag?: string;
}

declare interface NotificationPermissionCallback {
  (permission: NotificationPermission): void;
}

declare interface NotificationEventMap {
  click: Event;
  close: Event;
  error: Event;
  show: Event;
}

declare interface Notification extends EventTarget {
  readonly title: string;
  readonly body: string;
  readonly data: any;
  readonly dir: NotificationDirection;
  readonly icon: string;
  readonly lang: string;
  readonly badge: string;
  readonly tag: string;
  readonly silent: boolean | null;
  readonly requireInteraction: boolean;

  onclick: ((this: Notification, ev: Event) => any) | null;
  onclose: ((this: Notification, ev: Event) => any) | null;
  onerror: ((this: Notification, ev: Event) => any) | null;
  onshow: ((this: Notification, ev: Event) => any) | null;

  close(): void;

  addEventListener<K extends keyof NotificationEventMap>(
    type: K,
    listener: (this: Notification, ev: NotificationEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof NotificationEventMap>(
    type: K,
    listener: (this: Notification, ev: NotificationEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** Web Notifications API.
 *
 * Construct a notification to display it. Only available in apps
 * compiled with `deno desktop`.
 *
 * Notification permission is checked against the OS (e.g. macOS User
 * Notifications). {@linkcode Notification.permission} reports the
 * cached result of the most recent query/request, and
 * {@linkcode Notification.requestPermission} triggers a system prompt
 * if the user has not yet decided.
 *
 * The Web Notifications API specifies `icon` as a URL string. The
 * desktop runtime can only resolve `data:` URLs synchronously; other
 * URL schemes are accepted (the value round-trips through the
 * {@linkcode Notification.icon} property) but the OS notification is
 * shown without an icon.
 */
declare var Notification: {
  prototype: Notification;
  new (title: string, options?: NotificationOptions): Notification;
  readonly permission: NotificationPermission;
  readonly maxActions: number;
  requestPermission(
    deprecatedCallback?: NotificationPermissionCallback,
  ): Promise<NotificationPermission>;
};

/** Permissions API state value. Mirrors the Web Permissions API. */
declare type PermissionState = "granted" | "denied" | "prompt";

declare interface PermissionStatusEventMap {
  change: Event;
}

declare interface PermissionStatus extends EventTarget {
  readonly name: string;
  readonly state: PermissionState;
  onchange: ((this: PermissionStatus, ev: Event) => any) | null;

  addEventListener<K extends keyof PermissionStatusEventMap>(
    type: K,
    listener: (
      this: PermissionStatus,
      ev: PermissionStatusEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof PermissionStatusEventMap>(
    type: K,
    listener: (
      this: PermissionStatus,
      ev: PermissionStatusEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

declare var PermissionStatus: {
  prototype: PermissionStatus;
};

declare interface PermissionDescriptor {
  name: string;
}

declare interface Permissions {
  query(descriptor: PermissionDescriptor): Promise<PermissionStatus>;
}

/** Extends the {@linkcode Navigator} provided by `deno.window` with the
 * Permissions API surface available to `deno desktop` apps. */
declare interface Navigator {
  readonly permissions: Permissions;
}

declare namespace Deno.desktop {
  export {}; // stop default export type behavior

  /** The application version read from `deno.json` at compile time, or
   * `null` if no version was configured. Only available in apps compiled
   * with `deno desktop`. */
  export const appVersion: string | null;

  export interface AutoUpdateOptions {
    /** Base URL of the release server hosting `latest.json` and patch
     * files. Defaults to `desktop.release.baseUrl` from `deno.json` when
     * configured; required otherwise.
     *
     * Must be an `https:` URL — non-HTTPS URLs are refused. */
    url?: string;
    /** Poll interval in milliseconds. If omitted, only a single check is
     * performed ~1s after the call; pass an interval to keep checking for
     * the lifetime of the process. */
    interval?: number;
    /** Base64-encoded 32-byte Ed25519 public key used to verify the
     * release manifest.
     *
     * When set, `latest.json` must carry a top-level `signature` (base64
     * Ed25519 signature) over a `signed` field holding the manifest JSON
     * as a string. The signature is verified before any patch is fetched,
     * and only the contents of the verified `signed` payload are trusted.
     * A manifest that is unsigned or fails verification is rejected.
     *
     * Strongly recommended for production: without it, update integrity
     * rests solely on TLS and the per-patch SHA-256 in the manifest. */
    publicKey?: string;
    /** Called once an update has been downloaded, verified, and staged for
     * the next launch. Receives the version string being staged. */
    onUpdateReady?: (version: string) => void;
    /** Called if the previous launch's update failed to start and was
     * automatically rolled back to the prior version. Receives a
     * human-readable reason. */
    onRollback?: (reason: string) => void;
  }

  /** Start checking a release server for over-the-air updates.
   *
   * Updates are delivered as binary diffs against the app's native
   * library, so only the bytes that changed between versions are
   * downloaded. On each check, the manifest at `<url>/latest.json` is
   * fetched and its `version` compared against {@linkcode
   * Deno.desktop.appVersion}:
   *
   * - If `publicKey` is set, the manifest signature is verified first and
   *   an unsigned or invalid manifest is rejected.
   * - If the manifest advertises a newer version and lists a patch from
   *   the currently running version (under `patches[<currentVersion>]`,
   *   as `{ name, sha256 }`), that patch is downloaded, checked against
   *   its declared SHA-256, applied, and staged for the next launch.
   * - `onUpdateReady` is then invoked. The new version takes effect the
   *   next time the app starts.
   *
   * The staged update is swapped in atomically on the next launch, with
   * the previous version kept as a backup. If the updated app fails to
   * start, it is automatically rolled back to the backup and `onRollback`
   * is invoked shortly after the next `autoUpdate` call.
   *
   * The release server URL may be passed directly, supplied via
   * {@linkcode AutoUpdateOptions.url}, or configured once in `deno.json`
   * under `desktop.release.baseUrl` (in which case it can be omitted).
   * A single check runs ~1s after the call; pass `interval` to keep
   * polling.
   *
   * Because updates rewrite the app's own library file in place, they only
   * apply where the running process can write to its installed files. This
   * works for self-contained, user-writable installs (a `.app` bundle or a
   * loose binary in the user's home directory, a writable AppImage next to
   * its data). It does **not** work for read-only or system-owned installs
   * — an AppImage mounted read-only, or an app installed under `/usr` from
   * an `rpm`/`deb` package owned by `root`. In those cases the write fails,
   * the failure is logged, and the update is skipped; distribute updates
   * through the system package manager instead.
   *
   * Only available in apps compiled with `deno desktop`.
   *
   * ```ts
   * Deno.desktop.autoUpdate({
   *   url: "https://releases.example.com/myapp",
   *   interval: 60 * 60 * 1000, // hourly
   *   publicKey: "b64EncodedEd25519PublicKey==",
   *   onUpdateReady(version) {
   *     console.log(`v${version} staged; restart to apply`);
   *   },
   *   onRollback(reason) {
   *     console.warn(`update rolled back: ${reason}`);
   *   },
   * });
   * ```
   */
  export function autoUpdate(url: string): void;
  export function autoUpdate(options?: AutoUpdateOptions): void;

  export interface OpenDevtoolsOptions {
    /** Inspect the CEF renderer isolate. @default {true} */
    renderer?: boolean;
    /** Inspect the Deno runtime isolate. @default {true} */
    deno?: boolean;
  }

  export interface BrowserWindowOptions {
    title?: string;
    /** @default {800} */
    width?: number;
    /** @default {600} */
    height?: number;
    x?: number;
    y?: number;
    /** @default {true} */
    resizable?: boolean;
    /** @default {false} */
    alwaysOnTop?: boolean;
    /** Overall window opacity as a uniform factor in the range `0`–`1`, where
     * `1` is fully opaque (the default) and `0` is fully transparent. Fades the
     * entire window — web content and native chrome alike — like CSS `opacity`.
     * This is distinct from {@linkcode transparent}, which makes the background
     * transparent while honoring the page's own per-pixel alpha. Out-of-range
     * values are clamped. Can also be changed at runtime with
     * {@linkcode BrowserWindow.setOpacity}.
     *
     * @default {1} */
    opacity?: number;
    /** Remove the title bar and standard window chrome (border, traffic
     * light / caption buttons). Set at creation time only.
     *
     * @default {false} */
    frameless?: boolean;
    /** Create the window as a floating, non-activating utility "panel": it
     * floats above normal windows and does not activate the app or steal key
     * focus from the foreground app when shown. Combined with
     * {@linkcode frameless} and {@linkcode Tray.getBounds}, this is the
     * configuration used for tray / menu-bar popovers. Set at creation time
     * only.
     *
     * @default {false} */
    noActivate?: boolean;
    transparentTitlebar?: boolean;
    /** Give the window a transparent background so the web content's own alpha
     * composites against whatever is behind the window. Any region the page
     * leaves transparent (e.g. a `transparent` root background) shows the
     * desktop through it. Often combined with {@linkcode frameless}. Distinct
     * from {@linkcode opacity}, which uniformly fades the whole window. Set at
     * creation time only.
     *
     * Supported on macOS and Linux with the system WebView; ignored on Windows
     * and with the CEF backend, which paint an opaque window background.
     *
     * @default {false} */
    transparent?: boolean;
  }

  interface BrowserWindowObject {
    [key: string]: BrowserWindowValue;
  }

  type BrowserWindowValue =
    | null
    | boolean
    | number
    | string
    | BrowserWindowObject
    | BrowserWindowValue[]
    | Uint8Array;

  /** The leaf types that survive the trip across the webview boundary. */
  type BrowserWindowLeaf =
    | null
    | undefined
    | void
    | boolean
    | number
    | string
    | Uint8Array;

  /** Maps `T` to itself if every value it can hold survives the trip across
   * the webview boundary, and to `never` at the first member that does not.
   * `T` is serializable when `[T] extends [BrowserWindowSerializable<T>]`.
   *
   * This is a structural walk rather than a plain union because TypeScript
   * never gives an `interface` an implicit index signature: a union arm of
   * `{ [key: string]: unknown }` would reject every user-declared interface,
   * including `Deno.FileInfo`. Recursing through properties instead accepts
   * interfaces, and rejects the values that silently serialize to `{}` —
   * `Date`, `Map`, `Set`, class instances with methods, and functions.
   *
   * `unknown extends T` holds only for `unknown` and `any`. Both are let
   * through: nothing can be proven about them, and rejecting them would
   * break `Record<string, unknown>` payloads.
   *
   * `undefined` (and optional) properties are dropped during serialization,
   * and a handler that returns nothing resolves as `null` in the webview. */
  type BrowserWindowSerializable<T> = unknown extends T ? T
    : T extends BrowserWindowLeaf ? T
    : T extends (...args: any[]) => any ? never
    : T extends readonly (infer U)[] ? readonly BrowserWindowSerializable<U>[]
    : T extends object ? { [K in keyof T]: BrowserWindowSerializable<T[K]> }
    : never;

  /** The default bindings type: any set of async handlers.
   *
   * Handler parameters are `any[]` because the webview may call a binding
   * with arbitrary arguments — nothing checks them at runtime. Supply an
   * explicit type argument to {@linkcode BrowserWindow} to have the
   * arguments of {@linkcode BrowserWindow.bind} handlers checked and
   * inferred. Handler return values are checked either way. */
  export type WindowBindings = Record<
    string,
    (this: BrowserWindow, ...args: any[]) => Promise<unknown>
  >;

  /** Intersected with a handler's own type to reject non-serializable return
   * values at the call site of {@linkcode BrowserWindow.bind}.
   *
   * The check lives here, in parameter position, rather than as an
   * `R extends BrowserWindowSerializable<R>` type-parameter constraint,
   * because a type parameter constrained by a conditional type over itself
   * is circular (TS2313). Intersecting the string makes the mismatch print
   * the reason. */
  type BrowserWindowSerializableCheck<F> = F extends
    (...args: any[]) => Promise<infer P>
    ? [P] extends [BrowserWindowSerializable<P>] ? unknown
    : "binding handler must resolve with a serializable value"
    : unknown;

  /** Constrains T to a record of async binding functions taking and
   * resolving with serializable values.
   *
   * Each handler is validated and then passed through unchanged, so that
   * {@linkcode BrowserWindow.bind} keeps the caller's own parameter types.
   * Replacing `T[K]` with a common supertype here would instead force every
   * handler's parameters to be checked contravariantly against that
   * supertype, rejecting any handler that narrows them. */
  type ValidBindings<T> = {
    [K in keyof T]: T[K] extends (...args: infer A) => Promise<infer P>
      ? [A, P] extends
        [BrowserWindowSerializable<A>, BrowserWindowSerializable<P>] ? T[K]
      : never
      : never;
  };

  export type MenuItem =
    | {
      item: {
        label: string;
        id?: string;
        accelerator?: string;
        enabled: boolean;
      };
    }
    | {
      submenu: {
        label: string;
        items: MenuItem[];
      };
    }
    | "separator"
    | {
      role: {
        role: string;
      };
    };

  interface BrowserWindowResizeDetail {
    width: number;
    height: number;
  }

  interface BrowserWindowMoveDetail {
    x: number;
    y: number;
  }

  interface MenuClickDetail {
    id: string;
  }

  interface BrowserWindowEventMap {
    keydown: KeyboardEvent;
    keyup: KeyboardEvent;
    mousedown: MouseEvent;
    mouseup: MouseEvent;
    click: MouseEvent;
    dblclick: MouseEvent;
    mousemove: MouseEvent;
    mouseenter: MouseEvent;
    mouseleave: MouseEvent;
    wheel: WheelEvent;
    focus: FocusEvent;
    blur: FocusEvent;

    // non-standard events
    resize: CustomEvent<BrowserWindowResizeDetail>;
    move: CustomEvent<BrowserWindowMoveDetail>;
    close: Event;
    menuclick: CustomEvent<MenuClickDetail>;
    contextmenuclick: CustomEvent<MenuClickDetail>;
  }

  type BrowserWindowEventHandlers = {
    [K in keyof BrowserWindowEventMap as `on${K}`]:
      | ((this: BrowserWindow, ev: BrowserWindowEventMap[K]) => any)
      | null;
  };

  export interface BrowserWindow<T extends ValidBindings<T> = WindowBindings>
    extends BrowserWindowEventHandlers {}

  export class BrowserWindow<
    T extends ValidBindings<T> = WindowBindings,
  > extends EventTarget {
    constructor(options?: BrowserWindowOptions);

    readonly windowId: number;

    /** Expose `fn` to the webview under `name`.
     *
     * The resolved value must be serializable; see
     * {@linkcode BrowserWindowSerializable}. This is checked even when `T` is
     * left to its default, so returning e.g. a `Date` is a type error rather
     * than an empty object at runtime. */
    bind<N extends keyof T, F extends T[N]>(
      name: N,
      fn: F & BrowserWindowSerializableCheck<F>,
    ): void;
    unbind<N extends keyof T>(name: N): void;
    /** @throws {BrowserWindowValue} */
    executeJs(script: string): Promise<BrowserWindowValue>;

    setTitle(title: string): void;

    getSize(): [number, number];
    setSize(width: number, height: number): void;

    getPosition(): [number, number];
    setPosition(x: number, y: number): void;

    isResizable(): boolean;
    setResizable(resizable: boolean): void;

    isAlwaysOnTop(): boolean;
    setAlwaysOnTop(alwaysOnTop: boolean): void;

    /** Get the window's overall opacity, a uniform factor in the range `0`–`1`
     * where `1` is fully opaque. */
    getOpacity(): number;
    /** Set the window's overall opacity, a uniform factor in the range `0`–`1`
     * where `1` is fully opaque (the default) and `0` is fully transparent.
     * Fades the entire window — web content and native chrome alike — like CSS
     * `opacity`. Out-of-range values are clamped. No-op on backends without
     * opacity support. */
    setOpacity(opacity: number): void;

    isClosed(): boolean;
    close(): void;

    isVisible(): boolean;
    show(): void;
    hide(): void;
    focus(): void;
    navigate(url: string): void;
    /** Open a DevTools window.
     *
     * By default both targets are shown. Pass an options object to
     * select which targets to inspect. At least one must be `true`.
     */
    openDevtools(options?: OpenDevtoolsOptions): void;
    reload(): void;

    setApplicationMenu(menu: MenuItem[]): void;
    showContextMenu(x: number, y: number, menu: MenuItem[]): void;

    getNativeWindow(): Deno.UnsafeWindowSurface;

    addEventListener<K extends keyof BrowserWindowEventMap>(
      type: K,
      listener: (this: BrowserWindow, ev: BrowserWindowEventMap[K]) => any,
      options?: boolean | AddEventListenerOptions,
    ): void;
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ): void;
    removeEventListener<K extends keyof BrowserWindowEventMap>(
      type: K,
      listener: (this: BrowserWindow, ev: BrowserWindowEventMap[K]) => any,
      options?: boolean | EventListenerOptions,
    ): void;
    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ): void;
  }

  interface DockReopenDetail {
    hasVisibleWindows: boolean;
  }

  interface DockEventMap {
    menuclick: CustomEvent<MenuClickDetail>;
    reopen: CustomEvent<DockReopenDetail>;
  }

  type DockEventHandlers = {
    [K in keyof DockEventMap as `on${K}`]:
      | ((this: Dock, ev: DockEventMap[K]) => any)
      | null;
  };

  export interface Dock extends DockEventHandlers {}

  /** App-level dock / taskbar handle.
   *
   * A `"reopen"` event fires on macOS when the user clicks the dock icon;
   * the default behavior of showing the last hidden window is swallowed,
   * so listeners decide what (if anything) to do.
   */
  export class Dock extends EventTarget {
    constructor();

    /** Set a short text badge on the app's dock icon (macOS) or taskbar
     * icon (Windows), or prefix the focused window's title on Linux.
     * Pass `null` or an empty string to clear the badge. */
    setBadge(text: string | null): void;

    /** Bounce the dock icon (macOS), flash the focused window's taskbar
     * button (Windows), or set the urgency hint on the focused window
     * (Linux).
     *
     * When `critical` is `false` (the default) this triggers a single
     * bounce; when `true` it bounces continuously until the app is
     * focused. */
    bounce(critical?: boolean): void;

    /** Set a custom right-click menu on the app's dock icon. Pass
     * `null` to remove any menu previously set.
     *
     * macOS only. Click events are delivered as `"menuclick"` events on
     * {@linkcode Deno.desktop.dock}. No-op on Windows and Linux. */
    setMenu(menu: MenuItem[] | null): void;

    /** Show or hide the app's dock icon.
     *
     * macOS only — controls the app's activation policy. No-op on
     * Windows and Linux. */
    setVisible(visible: boolean): void;

    addEventListener<K extends keyof DockEventMap>(
      type: K,
      listener: (this: Dock, ev: DockEventMap[K]) => any,
      options?: boolean | AddEventListenerOptions,
    ): void;
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ): void;
    removeEventListener<K extends keyof DockEventMap>(
      type: K,
      listener: (this: Dock, ev: DockEventMap[K]) => any,
      options?: boolean | EventListenerOptions,
    ): void;
    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ): void;
  }

  /** App-level dock / taskbar singleton. */
  export const dock: Dock;

  /** The tray icon's bounding rectangle in screen coordinates, in the same
   * top-left-origin space as {@linkcode BrowserWindow.setPosition}. Use it to
   * anchor a popover window under the icon. */
  export interface TrayBounds {
    x: number;
    y: number;
    width: number;
    height: number;
  }

  export interface TrayPanelOptions {
    /** URL to load in the panel window. */
    url?: string;
    /** Panel width in pixels. @default {360} */
    width?: number;
    /** Panel height in pixels. @default {480} */
    height?: number;
    /** Hide the panel when it loses focus (click-outside to dismiss).
     * @default {true} */
    hideOnBlur?: boolean;
    /** Override where the panel is placed. Receives the tray icon's bounds
     * and the panel size, and returns the top-left screen position. The
     * default centers the panel horizontally under the icon — correct for a
     * top menu bar; provide this to place it elsewhere (e.g. above a
     * bottom-edge taskbar). */
    position?: (
      trayBounds: TrayBounds,
      panelSize: { width: number; height: number },
    ) => { x: number; y: number };
  }

  /** Handle to a tray-attached popover window created by
   * {@linkcode Tray.attachPanel}. */
  export interface TrayPanel {
    /** The underlying panel window — use it to `bind()`, `executeJs()`,
     * open devtools, etc. */
    readonly window: BrowserWindow;
    /** Whether the panel is currently shown. */
    readonly visible: boolean;
    /** Show the panel, positioned under the tray icon. */
    show(): void;
    /** Hide the panel. */
    hide(): void;
    /** Toggle the panel's visibility. */
    toggle(): void;
    /** Detach the panel: remove the tray/blur listeners and close the
     * window. */
    destroy(): void;
  }

  interface TrayEventMap {
    click: MouseEvent;
    dblclick: MouseEvent;
    menuclick: CustomEvent<MenuClickDetail>;
  }

  type TrayEventHandlers = {
    [K in keyof TrayEventMap as `on${K}`]:
      | ((this: Tray, ev: TrayEventMap[K]) => any)
      | null;
  };

  export interface Tray extends TrayEventHandlers {}

  /** A persistent icon in the OS status area (macOS menu bar extras,
   * Windows system tray, Linux AppIndicator).
   *
   * The icon is removed from the OS when {@linkcode Tray.destroy} is
   * called. Multiple trays may be created.
   */
  export class Tray extends EventTarget implements Disposable {
    constructor();

    readonly trayId: number;

    /** Set the tray icon image from PNG-encoded bytes. */
    setIcon(pngBytes: Uint8Array): void;

    /** Set the tray icon used in OS dark mode. Pass `null` to clear
     * it. */
    setIconDark(pngBytes: Uint8Array | null): void;

    /** Set the tooltip shown on hover. Pass `null` or an empty string
     * to clear the tooltip. */
    setTooltip(text: string | null): void;

    /** Set the right-click context menu. Click events are delivered as
     * `"menuclick"` events on the tray. Pass `null` to remove any
     * menu previously set. */
    setMenu(menu: MenuItem[] | null): void;

    /** The tray icon's bounding rectangle in screen coordinates, or `null`
     * if the icon has no on-screen position yet or the platform can't report
     * it. Typically called from a `"click"` handler to position a popover
     * {@linkcode BrowserWindow} (created with `frameless` + `noActivate`)
     * under the icon. */
    getBounds(): TrayBounds | null;

    /** Attach a frameless, non-activating popover window to this tray icon
     * (the classic menu-bar-app pattern). The returned panel toggles on tray
     * click, is positioned under the icon via {@linkcode Tray.getBounds}, and
     * hides when it loses focus.
     *
     * Convenience built on the primitives; for full control create a
     * `frameless` + `noActivate` {@linkcode BrowserWindow} yourself.
     *
     * ```ts
     * const tray = new Deno.desktop.Tray();
     * tray.setIcon(iconBytes);
     * const panel = tray.attachPanel({ url: "https://localhost:8000/panel" });
     * panel.window.bind("doThing", async () => { ... });
     * ```
     *
     * Pass a string as shorthand for `{ url }`. On Linux the icon position
     * can't be queried, so the panel shows at its last position rather than
     * anchored to the icon. */
    attachPanel(options: TrayPanelOptions | string): TrayPanel;

    /** Remove the tray icon from the OS status area. The instance must
     * not be used after this call. */
    destroy(): void;

    [Symbol.dispose](): void;

    addEventListener<K extends keyof TrayEventMap>(
      type: K,
      listener: (this: Tray, ev: TrayEventMap[K]) => any,
      options?: boolean | AddEventListenerOptions,
    ): void;
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ): void;
    removeEventListener<K extends keyof TrayEventMap>(
      type: K,
      listener: (this: Tray, ev: TrayEventMap[K]) => any,
      options?: boolean | EventListenerOptions,
    ): void;
    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ): void;
  }
}
