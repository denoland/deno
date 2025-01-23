// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category I/O */
/**
 * The Console interface provides methods for logging information to the console,
 * as well as other utility methods for debugging and inspecting code.
 * @see https://developer.mozilla.org/en-US/docs/Web/API/console
 */
/** Interface representing the console object that provides methods for logging, debugging, and timing */
interface Console {
  /**
   * Tests that an expression is true. If not, logs an error message
   * @param condition The expression to test for truthiness
   * @param data Additional arguments to be printed if the assertion fails
   * @example
   * ```ts
   * console.assert(1 === 1, "This won't show");
   * console.assert(1 === 2, "This will show an error");
   * ```
   */
  assert(condition?: boolean, ...data: any[]): void;

  /**
   * Clears the console if the environment allows it
   * @example
   * ```ts
   * console.clear();
   * ```
   */
  clear(): void;

  /**
   * Maintains an internal counter for a given label, incrementing it each time the method is called
   * @param label The label to count. Defaults to 'default'
   * @example
   * ```ts
   * console.count('myCounter');
   * console.count('myCounter'); // Will show: myCounter: 2
   * ```
   */
  count(label?: string): void;

  /**
   * Resets the counter for a given label
   * @param label The label to reset. Defaults to 'default'
   * @example
   * ```ts
   * console.count('myCounter');
   * console.countReset('myCounter'); // Resets to 0
   * ```
   */
  countReset(label?: string): void;

  /**
   * Outputs a debugging message to the console
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.debug('Debug message', { detail: 'some data' });
   * ```
   */
  debug(...data: any[]): void;

  /**
   * Displays a list of the properties of a specified object
   * @param item Object to display
   * @param options Formatting options
   * @example
   * ```ts
   * console.dir({ name: 'object', value: 42 }, { depth: 1 });
   * ```
   */
  dir(item?: any, options?: any): void;

  /**
   * @ignore
   */
  dirxml(...data: any[]): void;

  /**
   * Outputs an error message to the console.
   * This method routes the output to stderr,
   * unlike other console methods that route to stdout.
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.error('Error occurred:', new Error('Something went wrong'));
   * ```
   */
  error(...data: any[]): void;

  /**
   * Creates a new inline group in the console, indenting subsequent console messages
   * @param data Labels for the group
   * @example
   * ```ts
   * console.group('Group 1');
   * console.log('Inside group 1');
   * console.groupEnd();
   * ```
   */
  group(...data: any[]): void;

  /**
   * Creates a new inline group in the console that is initially collapsed
   * @param data Labels for the group
   * @example
   * ```ts
   * console.groupCollapsed('Details');
   * console.log('Hidden until expanded');
   * console.groupEnd();
   * ```
   */
  groupCollapsed(...data: any[]): void;

  /**
   * Exits the current inline group in the console
   * @example
   * ```ts
   * console.group('Group');
   * console.log('Grouped message');
   * console.groupEnd();
   * ```
   */
  groupEnd(): void;

  /**
   * Outputs an informational message to the console
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.info('Application started', { version: '1.0.0' });
   * ```
   */
  info(...data: any[]): void;

  /**
   * Outputs a message to the console
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.log('Hello', 'World', 123);
   * ```
   */
  log(...data: any[]): void;

  /**
   * Displays tabular data as a table
   * @param tabularData Data to be displayed in table format
   * @param properties Array of property names to be displayed
   * @example
   * ```ts
   * console.table([
   *   { name: 'John', age: 30 },
   *   { name: 'Jane', age: 25 }
   * ]);
   * ```
   */
  table(tabularData?: any, properties?: string[]): void;

  /**
   * Starts a timer you can use to track how long an operation takes
   * @param label Timer label. Defaults to 'default'
   * @example
   * ```ts
   * console.time('operation');
   * // ... some code
   * console.timeEnd('operation');
   * ```
   */
  time(label?: string): void;

  /**
   * Stops a timer that was previously started
   * @param label Timer label to stop. Defaults to 'default'
   * @example
   * ```ts
   * console.time('operation');
   * // ... some code
   * console.timeEnd('operation'); // Prints: operation: 1234ms
   * ```
   */
  timeEnd(label?: string): void;

  /**
   * Logs the current value of a timer that was previously started
   * @param label Timer label
   * @param data Additional data to log
   * @example
   * ```ts
   * console.time('process');
   * // ... some code
   * console.timeLog('process', 'Checkpoint A');
   * ```
   */
  timeLog(label?: string, ...data: any[]): void;

  /**
   * Outputs a stack trace to the console
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.trace('Trace message');
   * ```
   */
  trace(...data: any[]): void;

  /**
   * Outputs a warning message to the console
   * @param data Values to be printed to the console
   * @example
   * ```ts
   * console.warn('Deprecated feature used');
   * ```
   */
  warn(...data: any[]): void;

  /**
   * Adds a marker to the DevTools Performance panel
   * @param label Label for the timestamp
   * @example
   * ```ts
   * console.timeStamp('Navigation Start');
   * ```
   */
  timeStamp(label?: string): void;

  /**
   * Starts recording a performance profile
   * @param label Profile label
   * @example
   * ```ts
   * console.profile('Performance Profile');
   * // ... code to profile
   * console.profileEnd('Performance Profile');
   * ```
   */
  profile(label?: string): void;

  /**
   * Stops recording a performance profile
   * @param label Profile label to stop
   * @example
   * ```ts
   * console.profile('Performance Profile');
   * // ... code to profile
   * console.profileEnd('Performance Profile');
   * ```
   */
  profileEnd(label?: string): void;
}
