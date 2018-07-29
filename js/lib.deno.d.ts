// Copyright 2018 the Deno authors. All rights reserved. MIT license.

// This file contains the default TypeScript libraries for the runtime

/// <reference no-default-lib="true"/>

/// <reference lib="esnext" />

// This needs to be stripped down to what is supported by V8 without a DOM
interface Console {
  memory: any;
  assert(condition?: boolean, message?: string, ...data: any[]): void;
  clear(): void;
  count(label?: string): void;
  debug(message?: any, ...optionalParams: any[]): void;
  dir(value?: any, ...optionalParams: any[]): void;
  dirxml(value: any): void;
  error(message?: any, ...optionalParams: any[]): void;
  exception(message?: string, ...optionalParams: any[]): void;
  group(groupTitle?: string, ...optionalParams: any[]): void;
  groupCollapsed(groupTitle?: string, ...optionalParams: any[]): void;
  groupEnd(): void;
  info(message?: any, ...optionalParams: any[]): void;
  log(message?: any, ...optionalParams: any[]): void;
  markTimeline(label?: string): void;
  // msIsIndependentlyComposed(element: Element): boolean;
  profile(reportName?: string): void;
  profileEnd(): void;
  // select(element: Element): void;
  table(...tabularData: any[]): void;
  time(label?: string): void;
  timeEnd(label?: string): void;
  timeStamp(label?: string): void;
  timeline(label?: string): void;
  timelineEnd(label?: string): void;
  trace(message?: any, ...optionalParams: any[]): void;
  warn(message?: any, ...optionalParams: any[]): void;
}

declare var Console: {
    prototype: Console;
    new(): Console;
};

interface Window {
  console: Console;
}

// Globals in the runtime environment
declare let console: Console;
declare const window: Window;
