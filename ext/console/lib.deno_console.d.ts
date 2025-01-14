// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category I/O */
/**
 * The Console interface provides methods for logging information to the console,
 * as well as other utility methods for debugging and inspecting code.
 * @see https://developer.mozilla.org/en-US/docs/Web/API/console
 * 
 * @interface Console
 * 
 * @method assert
 * @param {boolean} [condition] - The condition to assert.
 * @param {...any[]} data - Data to log if the assertion fails.
 * @example
 * console.assert(1 === 1, "This will not log");
 * console.assert(1 === 2, "This will log");
 * 
 * @method clear
 * Clears the console.
 * @example
 * console.clear();
 * 
 * @method count
 * @param {string} [label] - The label for the counter.
 * @example
 * console.count("myLabel");
 * console.count("myLabel");
 * 
 * @method countReset
 * @param {string} [label] - The label for the counter to reset.
 * @example
 * console.count("myLabel");
 * console.countReset("myLabel");
 * console.count("myLabel");
 * 
 * @method debug
 * @param {...any[]} data - Data to log at the debug level.
 * @example
 * console.debug("Debug message");
 * 
 * @method dir
 * @param {any} [item] - The item to log.
 * @param {any} [options] - Options for logging.
 * @example
 * console.dir({key: "value"});
 * 
 * @method dirxml
 * @param {...any[]} data - Data to log as XML.
 * @example
 * console.dirxml(document);
 * 
 * @method error
 * @param {...any[]} data - Data to log at the error level.
 * @example
 * console.error("Error message");
 * 
 * @method group
 * @param {...any[]} data - Data to log in a group.
 * @example
 * console.group("Group label");
 * console.log("Inside group");
 * console.groupEnd();
 * 
 * @method groupCollapsed
 * @param {...any[]} data - Data to log in a collapsed group.
 * @example
 * console.groupCollapsed("Collapsed group label");
 * console.log("Inside collapsed group");
 * console.groupEnd();
 * 
 * @method groupEnd
 * Ends the current group.
 * @example
 * console.group("Group label");
 * console.log("Inside group");
 * console.groupEnd();
 * 
 * @method info
 * @param {...any[]} data - Data to log at the info level.
 * @example
 * console.info("Info message");
 * 
 * @method log
 * @param {...any[]} data - Data to log.
 * @example
 * console.log("Log message");
 * 
 * @method table
 * @param {any} [tabularData] - Data to log as a table.
 * @param {string[]} [properties] - Properties to include in the table.
 * @example
 * console.table([{a: 1, b: 2}, {a: 3, b: 4}]);
 * 
 * @method time
 * @param {string} [label] - The label for the timer.
 * @example
 * console.time("myTimer");
 * 
 * @method timeEnd
 * @param {string} [label] - The label for the timer to end.
 * @example
 * console.time("myTimer");
 * console.timeEnd("myTimer");
 * 
 * @method timeLog
 * @param {string} [label] - The label for the timer.
 * @param {...any[]} data - Data to log with the timer.
 * @example
 * console.time("myTimer");
 * console.timeLog("myTimer", "Additional data");
 * 
 * @method trace
 * @param {...any[]} data - Data to log with a stack trace.
 * @example
 * console.trace("Trace message");
 * 
 * @method warn
 * @param {...any[]} data - Data to log at the warn level.
 * @example
 * console.warn("Warning message");
 * 
 * @method timeStamp
 * @param {string} [label] - The label for the timestamp.
 * @example
 * console.timeStamp("myTimestamp");
 * 
 * @method profile
 * @param {string} [label] - The label for the profile.
 * @example
 * console.profile("myProfile");
 * 
 * @method profileEnd
 * @param {string} [label] - The label for the profile to end.
 * @example
 * console.profile("myProfile");
 * console.profileEnd("myProfile");
 */
interface Console {
  assert(condition?: boolean, ...data: any[]): void;
  clear(): void;
  count(label?: string): void;
  countReset(label?: string): void;
  debug(...data: any[]): void;
  dir(item?: any, options?: any): void;
  dirxml(...data: any[]): void;
  error(...data: any[]): void;
  group(...data: any[]): void;
  groupCollapsed(...data: any[]): void;
  groupEnd(): void;
  info(...data: any[]): void;
  log(...data: any[]): void;
  table(tabularData?: any, properties?: string[]): void;
  time(label?: string): void;
  timeEnd(label?: string): void;
  timeLog(label?: string, ...data: any[]): void;
  trace(...data: any[]): void;
  warn(...data: any[]): void;

  /** This method is a noop, unless used in inspector */
  timeStamp(label?: string): void;

  /** This method is a noop, unless used in inspector */
  profile(label?: string): void;

  /** This method is a noop, unless used in inspector */
  profileEnd(label?: string): void;
}
