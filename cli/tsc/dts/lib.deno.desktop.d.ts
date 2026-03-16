// Copyright 2018-2026 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="esnext" />
/// <reference lib="es2022.intl" />

// TODO: hook this file up

declare namespace Deno {
  export {}; // stop default export type behavior

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

  interface BrowserWindowEventMap {
    keydown: KeyboardEvent;
    keyup: KeyboardEvent;
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

    bind<N extends keyof T>(name: N, fn: T[N]): void;
    unbind<N extends keyof T>(name: N): void;
    /** @throws {BrowserWindowValue} */
    executeJs(script: string): Promise<BrowserWindowValue>;

    setTitle(title: string);

    getSize(): [number, number];
    setSize(width: number, height: number);

    getPosition(): [number, number];
    setPosition(x: number, y: number);

    isResizable(): boolean;
    setResizable(resizable: boolean);

    isAlwaysOnTop(): boolean;
    setAlwaysOnTop(alwaysOnTop: boolean);

    isClosed(): boolean;
    close(): void;

    isVisible(): boolean;
    show(): void;
    hide(): void;
    focus(): void;
    navigate(url: string): void;
    reload(): void;

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
}
