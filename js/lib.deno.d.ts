// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This file contains the default TypeScript libraries for the runtime

/// <reference no-default-lib="true"/>

/// <reference lib="esnext" />

// TODO generate `console.d.ts` and inline it in `assets.ts` and remove
// declaration of `Console`
// import { Console } from 'gen/console';

declare class Console {
  // tslint:disable-next-line:no-any
  log(...args: any[]): void;
  // tslint:disable-next-line:no-any
  debug(...args: any[]): void;
  // tslint:disable-next-line:no-any
  info(...args: any[]): void;
  // tslint:disable-next-line:no-any
  warn(...args: any[]): void;
  // tslint:disable-next-line:no-any
  error(...args: any[]): void;
  // tslint:disable-next-line:no-any
  assert(condition: boolean, ...args: any[]): void;
}

declare function clearInterval(handle?: number): void;
declare function clearTimeout(handle?: number): void;
declare function setInterval(
  handler: (...args: any[]) => void,
  timeout: number
): number;
declare function setInterval(
  handler: any,
  timeout?: any,
  ...args: any[]
): number;
declare function setTimeout(
  handler: (...args: any[]) => void,
  timeout: number
): number;
declare function setTimeout(
  handler: any,
  timeout?: any,
  ...args: any[]
): number;
declare function clearImmediate(handle: number): void;
declare function setImmediate(handler: (...args: any[]) => void): number;
declare function setImmediate(handler: any, ...args: any[]): number;

interface WindowTimers extends WindowTimersExtension {
  clearInterval(handle?: number): void;
  clearTimeout(handle?: number): void;
  setInterval(handler: (...args: any[]) => void, timeout: number): number;
  setInterval(handler: any, timeout?: any, ...args: any[]): number;
  setTimeout(handler: (...args: any[]) => void, timeout: number): number;
  setTimeout(handler: any, timeout?: any, ...args: any[]): number;
}

interface WindowTimersExtension {
  // TODO(ry) Yet unimplemented.
  clearImmediate(handle: number): void;
  setImmediate(handler: (...args: any[]) => void): number;
  setImmediate(handler: any, ...args: any[]): number;
}

interface Window extends WindowTimers {
  console: Console;

  // TODO(ry) These shouldn't be global.
  mainSource: string;
  setMainSourceMap(sm: string): void;
}

// Globals in the runtime environment
declare let console: Console;
declare const window: Window;

// TODO(ry) These shouldn't be global.
declare let mainSource: string;
declare function setMainSourceMap(sm: string): void;
