// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
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

declare namespace Deno {
  export {}; // stop default export type behavior

  /** The application version read from `deno.json` at compile time, or
   * `null` if no version was configured. Only available in apps compiled
   * with `deno desktop`. */
  export const desktopVersion: string | null;

  export interface AutoUpdateOptions {
    /** Base URL of the release server hosting `latest.json` and patch
     * files. Required unless a string URL is passed directly to
     * {@linkcode Deno.autoUpdate}. */
    url?: string;
    /** Poll interval in milliseconds. If omitted, only a single check is
     * performed ~1s after the call. */
    interval?: number;
    /** Called once an update has been downloaded and staged for the next
     * launch. */
    onUpdateReady?: (version: string) => void;
    /** Called if the previous launch's update failed to start and was
     * rolled back. */
    onRollback?: (reason: string) => void;
  }

  /** Start polling a release server for binary-diff updates.
   *
   * The manifest at `<url>/latest.json` is fetched and compared against
   * {@linkcode Deno.desktopVersion}. If a newer version is available and
   * a patch from the current version exists, the patch is downloaded and
   * staged for the next launch, and `onUpdateReady` is invoked.
   *
   * If the previous launch's update failed and was rolled back,
   * `onRollback` is invoked shortly after this call.
   *
   * Only available in apps compiled with `deno desktop`. */
  export function autoUpdate(url: string): void;
  export function autoUpdate(options: AutoUpdateOptions): void;

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

  export type WindowBindings = Record<
    string,
    (
      this: BrowserWindow,
      ...args: BrowserWindowValue[]
    ) => Promise<BrowserWindowValue>
  >;

  /** Constrains T to a record of async binding functions. */
  type ValidBindings<T> = {
    [K in keyof T]: (
      this: BrowserWindow,
      ...args: BrowserWindowValue[]
    ) => Promise<BrowserWindowValue>;
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

    bind<N extends keyof T>(name: N, fn: T[N]): void;
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
     * `"informational"` (the default) triggers a single bounce;
     * `"critical"` bounces continuously until the app is focused. */
    bounce(type?: "informational" | "critical"): void;

    /** Set a custom right-click menu on the app's dock icon. Pass
     * `null` to remove any menu previously set.
     *
     * macOS only. Click events are delivered as `"menuclick"` events on
     * {@linkcode Deno.dock}. No-op on Windows and Linux. */
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
