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

interface Window {
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
