// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

/** A snapshotting library.
 *
 * The `assertSnapshot` function will create a snapshot of a value and compare it
 * to a reference snapshot, which is stored alongside the test file in the
 * `__snapshots__` directory.
 *
 * ```ts
 * // example_test.ts
 * import { assertSnapshot } from "https://deno.land/std@$STD_VERSION/testing/snapshot.ts";
 *
 * Deno.test("isSnapshotMatch", async function (t): Promise<void> {
 *   const a = {
 *     hello: "world!",
 *     example: 123,
 *   };
 *   await assertSnapshot(t, a);
 * });
 * ```
 *
 * ```js
 * // __snapshots__/example_test.ts.snap
 * export const snapshot = {};
 *
 * snapshot[`isSnapshotMatch 1`] = `
 * {
 *   example: 123,
 *   hello: "world!",
 * }
 * `;
 * ```
 *
 * Calling `assertSnapshot` in a test will throw an `AssertionError`, causing the
 * test to fail, if the snapshot created during the test does not match the one in
 * the snapshot file.
 *
 * ## Updating Snapshots:
 *
 * When adding new snapshot assertions to your test suite, or when intentionally
 * making changes which cause your snapshots to fail, you can update your snapshots
 * by running the snapshot tests in update mode. Tests can be run in update mode by
 * passing the `--update` or `-u` flag as an argument when running the test. When
 * this flag is passed, then any snapshots which do not match will be updated.
 *
 * ```sh
 * deno test --allow-all -- --update
 * ```
 *
 * Additionally, new snapshots will only be created when this flag is present.
 *
 * ## Permissions:
 *
 * When running snapshot tests, the `--allow-read` permission must be enabled, or
 * else any calls to `assertSnapshot` will fail due to insufficient permissions.
 * Additionally, when updating snapshots, the `--allow-write` permission must also
 * be enabled, as this is required in order to update snapshot files.
 *
 * The `assertSnapshot` function will only attempt to read from and write to
 * snapshot files. As such, the allow list for `--allow-read` and `--allow-write`
 * can be limited to only include existing snapshot files, if so desired.
 *
 * ## Options:
 *
 * The `assertSnapshot` function optionally accepts an options object.
 *
 * ```ts
 * // example_test.ts
 * import { assertSnapshot } from "https://deno.land/std@$STD_VERSION/testing/snapshot.ts";
 *
 * Deno.test("isSnapshotMatch", async function (t): Promise<void> {
 *   const a = {
 *     hello: "world!",
 *     example: 123,
 *   };
 *   await assertSnapshot(t, a, {
 *     // options
 *   });
 * });
 * ```
 *
 * You can also configure default options for `assertSnapshot`.
 *
 * ```ts
 * // example_test.ts
 * import { createAssertSnapshot } from "https://deno.land/std@$STD_VERSION/testing/snapshot.ts";
 *
 * const assertSnapshot = createAssertSnapshot({
 *   // options
 * });
 * ```
 *
 * When configuring default options like this, the resulting `assertSnapshot`
 * function will function the same as the default function exported from the
 * snapshot module. If passed an optional options object, this will take precedence
 * over the default options, where the value provided for an option differs.
 *
 * It is possible to "extend" an `assertSnapshot` function which has been
 * configured with default options.
 *
 * ```ts
 * // example_test.ts
 * import { createAssertSnapshot } from "https://deno.land/std@$STD_VERSION/testing/snapshot.ts";
 * import { stripColor } from "https://deno.land/std@$STD_VERSION/fmt/colors.ts";
 *
 * const assertSnapshot = createAssertSnapshot({
 *   dir: ".snaps",
 * });
 *
 * const assertMonochromeSnapshot = createAssertSnapshot<string>(
 *   { serializer: stripColor },
 *   assertSnapshot,
 * );
 *
 * Deno.test("isSnapshotMatch", async function (t): Promise<void> {
 *   const a = "\x1b[32mThis green text has had it's colours stripped\x1b[39m";
 *   await assertMonochromeSnapshot(t, a);
 * });
 * ```
 *
 * ```js
 * // .snaps/example_test.ts.snap
 * export const snapshot = {};
 *
 * snapshot[`isSnapshotMatch 1`] = `This green text has had it's colours stripped`;
 * ```
 *
 * ## Version Control:
 *
 * Snapshot testing works best when changes to snapshot files are committed
 * alongside other code changes. This allows for changes to reference snapshots to
 * be reviewed along side the code changes that caused them, and ensures that when
 * others pull your changes, their tests will pass without needing to update
 * snapshots locally.
 *
 * @module
 */

import { fromFileUrl } from "../path/from_file_url.ts";
import { parse } from "../path/parse.ts";
import { resolve } from "../path/resolve.ts";
import { toFileUrl } from "../path/to_file_url.ts";
import { ensureFile, ensureFileSync } from "../fs/ensure_file.ts";
import { bold, green, red } from "../fmt/colors.ts";
import { assert } from "../assert/assert.ts";
import { AssertionError } from "../assert/assertion_error.ts";
import { equal } from "../assert/equal.ts";
import { assertEquals } from "../assert/assert_equals.ts";

const SNAPSHOT_DIR = "__snapshots__";
const SNAPSHOT_EXT = "snap";

export type SnapshotMode = "assert" | "update";

export type SnapshotOptions<T = unknown> = {
  /**
   * Snapshot output directory. Snapshot files will be written to this directory.
   * This can be relative to the test directory or an absolute path.
   *
   * If both `dir` and `path` are specified, the `dir` option will be ignored and
   * the `path` option will be handled as normal.
   */
  dir?: string;
  /**
   * Snapshot mode. Defaults to `assert`, unless the `-u` or `--update` flag is
   * passed, in which case this will be set to `update`. This option takes higher
   * priority than the update flag. If the `--update` flag is passed, it will be
   * ignored if the `mode` option is set.
   */
  mode?: SnapshotMode;
  /**
   * Failure message to log when the assertion fails. Specifying this option will
   * cause the diff not to be logged.
   */
  msg?: string;
  /**
   * Name of the snapshot to use in the snapshot file.
   */
  name?: string;
  /**
   * Snapshot output path. The snapshot will be written to this file. This can be
   * a path relative to the test directory or an absolute path.
   *
   * If both `dir` and `path` are specified, the `dir` option will be ignored and
   * the `path` option will be handled as normal.
   */
  path?: string;
  /**
   * Function to use when serializing the snapshot.
   */
  serializer?: (actual: T) => string;
};

function getErrorMessage(message: string, options: SnapshotOptions) {
  return typeof options.msg === "string" ? options.msg : message;
}

/**
 * Default serializer for `assertSnapshot`.
 */
export function serialize(actual: unknown): string;
export function serialize<T>(actual: T): string;
export function serialize(actual: unknown): string {
  return Deno.inspect(actual, {
    depth: Infinity,
    sorted: true,
    trailingComma: true,
    compact: false,
    iterableLimit: Infinity,
    strAbbreviateSize: Infinity,
    breakLength: Infinity,
    escapeSequences: false,
  });
}

/**
 * Converts a string to a valid JavaScript string which can be wrapped in backticks.
 *
 * @example
 *
 * "special characters (\ ` $) will be escaped" -> "special characters (\\ \` \$) will be escaped"
 */
function escapeStringForJs(str: string) {
  return str
    .replace(/\\/g, "\\\\")
    .replace(/`/g, "\\`")
    .replace(/\$/g, "\\$");
}

let _mode: SnapshotMode;
/**
 * Get the snapshot mode.
 */
function getMode(options: SnapshotOptions) {
  if (options.mode) {
    return options.mode;
  } else if (_mode) {
    return _mode;
  } else {
    _mode = Deno.args.some((arg) => arg === "--update" || arg === "-u")
      ? "update"
      : "assert";
    return _mode;
  }
}

/**
 * Return `true` when snapshot mode is `update`.
 */
function getIsUpdate(options: SnapshotOptions) {
  return getMode(options) === "update";
}

class AssertSnapshotContext {
  static contexts = new Map<string, AssertSnapshotContext>();

  /**
   * Returns an instance of `AssertSnapshotContext`. This will be retrieved from
   * a cache if an instance was already created for a given snapshot file path.
   */
  static fromOptions(
    testContext: Deno.TestContext,
    options: SnapshotOptions,
  ): AssertSnapshotContext {
    let path: string;
    const testFilePath = fromFileUrl(testContext.origin);
    const { dir, base } = parse(testFilePath);
    if (options.path) {
      path = resolve(dir, options.path);
    } else if (options.dir) {
      path = resolve(dir, options.dir, `${base}.${SNAPSHOT_EXT}`);
    } else {
      path = resolve(dir, SNAPSHOT_DIR, `${base}.${SNAPSHOT_EXT}`);
    }

    let context = this.contexts.get(path);
    if (context) {
      return context;
    }

    context = new this(toFileUrl(path));
    this.contexts.set(path, context);
    return context;
  }

  #teardownRegistered = false;
  #currentSnapshots: Map<string, string | undefined> | undefined;
  #updatedSnapshots = new Map<string, string>();
  #snapshotCounts = new Map<string, number>();
  #snapshotsUpdated = new Array<string>();
  #snapshotFileUrl: URL;
  snapshotUpdateQueue = new Array<string>();

  constructor(snapshotFileUrl: URL) {
    this.#snapshotFileUrl = snapshotFileUrl;
  }

  /**
   * Asserts that `this.#currentSnapshots` has been initialized and then returns it.
   *
   * Should only be called when `this.#currentSnapshots` has already been initialized.
   */
  #getCurrentSnapshotsInitialized() {
    assert(
      this.#currentSnapshots,
      "Snapshot was not initialized. This is a bug in `assertSnapshot`.",
    );
    return this.#currentSnapshots;
  }

  /**
   * Write updates to the snapshot file and log statistics.
   */
  #teardown = () => {
    const buf = ["export const snapshot = {};"];
    const currentSnapshots = this.#getCurrentSnapshotsInitialized();
    const currentSnapshotNames = Array.from(currentSnapshots.keys());
    const removedSnapshotNames = currentSnapshotNames.filter((name) =>
      !this.snapshotUpdateQueue.includes(name)
    );
    this.snapshotUpdateQueue.forEach((name) => {
      const updatedSnapshot = this.#updatedSnapshots.get(name);
      const currentSnapshot = currentSnapshots.get(name);
      let formattedSnapshot: string;
      if (typeof updatedSnapshot === "string") {
        formattedSnapshot = updatedSnapshot;
      } else if (typeof currentSnapshot === "string") {
        formattedSnapshot = currentSnapshot;
      } else {
        // This occurs when `assertSnapshot` is called in "assert" mode but
        // the snapshot doesn't exist and `assertSnapshot` is also called in
        // "update" mode. In this case, we have nothing to write to the
        // snapshot file so we can just exit early
        return;
      }
      formattedSnapshot = escapeStringForJs(formattedSnapshot);
      formattedSnapshot = formattedSnapshot.includes("\n")
        ? `\n${formattedSnapshot}\n`
        : formattedSnapshot;
      const formattedName = escapeStringForJs(name);
      buf.push(`\nsnapshot[\`${formattedName}\`] = \`${formattedSnapshot}\`;`);
    });
    const snapshotFilePath = fromFileUrl(this.#snapshotFileUrl);
    ensureFileSync(snapshotFilePath);
    Deno.writeTextFileSync(snapshotFilePath, buf.join("\n") + "\n");

    const updated = this.getUpdatedCount();
    if (updated > 0) {
      console.log(
        green(
          bold(
            `\n > ${updated} ${
              updated === 1 ? "snapshot" : "snapshots"
            } updated.`,
          ),
        ),
      );
    }
    const removed = removedSnapshotNames.length;
    if (removed > 0) {
      console.log(
        red(
          bold(
            `\n > ${removed} ${
              removed === 1 ? "snapshot" : "snapshots"
            } removed.`,
          ),
        ),
      );
      for (const snapshotName of removedSnapshotNames) {
        console.log(
          red(bold(`   • ${snapshotName}`)),
        );
      }
    }
  };

  /**
   * Returns `this.#currentSnapshots` and if necessary, tries to initialize it by reading existing
   * snapshots from the snapshot file. If the snapshot mode is `update` and the snapshot file does
   * not exist then it will be created.
   */
  async #readSnapshotFile(options: SnapshotOptions) {
    if (this.#currentSnapshots) {
      return this.#currentSnapshots;
    }

    if (getIsUpdate(options)) {
      await ensureFile(fromFileUrl(this.#snapshotFileUrl));
    }

    try {
      const snapshotFileUrl = this.#snapshotFileUrl.toString();
      const { snapshot } = await import(snapshotFileUrl);
      this.#currentSnapshots = typeof snapshot === "undefined"
        ? new Map()
        : new Map(
          Object.entries(snapshot).map(([name, snapshot]) => {
            if (typeof snapshot !== "string") {
              throw new AssertionError(
                getErrorMessage(
                  `Corrupt snapshot:\n\t(${name})\n\t${snapshotFileUrl}`,
                  options,
                ),
              );
            }
            return [
              name,
              snapshot.includes("\n") ? snapshot.slice(1, -1) : snapshot,
            ];
          }),
        );
      return this.#currentSnapshots;
    } catch (error) {
      if (
        error instanceof TypeError &&
        error.message.startsWith("Module not found")
      ) {
        throw new AssertionError(
          getErrorMessage(
            "Missing snapshot file.",
            options,
          ),
        );
      }
      throw error;
    }
  }

  /**
   * Register a teardown function which writes the snapshot file to disk and logs the number
   * of snapshots updated after all tests have run.
   *
   * This method can safely be called more than once and will only register the teardown
   * function once in a context.
   */
  public registerTeardown() {
    if (!this.#teardownRegistered) {
      globalThis.addEventListener("unload", this.#teardown);
      this.#teardownRegistered = true;
    }
  }

  /**
   * Gets the number of snapshots which have been created with the same name and increments
   * the count by 1.
   */
  public getCount(snapshotName: string) {
    let count = this.#snapshotCounts.get(snapshotName) || 0;
    this.#snapshotCounts.set(snapshotName, ++count);
    return count;
  }

  /**
   * Get an existing snapshot by name or returns `undefined` if the snapshot does not exist.
   */
  public async getSnapshot(snapshotName: string, options: SnapshotOptions) {
    const snapshots = await this.#readSnapshotFile(options);
    return snapshots.get(snapshotName);
  }

  /**
   * Update a snapshot by name. Updates will be written to the snapshot file when all tests
   * have run. If the snapshot does not exist, it will be created.
   *
   * Should only be called when mode is `update`.
   */
  public updateSnapshot(snapshotName: string, snapshot: string) {
    if (!this.#snapshotsUpdated.includes(snapshotName)) {
      this.#snapshotsUpdated.push(snapshotName);
    }
    const currentSnapshots = this.#getCurrentSnapshotsInitialized();
    if (!currentSnapshots.has(snapshotName)) {
      currentSnapshots.set(snapshotName, undefined);
    }
    this.#updatedSnapshots.set(snapshotName, snapshot);
  }

  /**
   * Get the number of updated snapshots.
   */
  public getUpdatedCount() {
    return this.#snapshotsUpdated.length;
  }

  /**
   * Add a snapshot to the update queue.
   *
   * Tracks the order in which snapshots were created so that they can be written to
   * the snapshot file in the correct order.
   *
   * Should be called with each snapshot, regardless of the mode, as a future call to
   * `assertSnapshot` could cause updates to be written to the snapshot file if the
   * `update` mode is passed in the options.
   */
  public pushSnapshotToUpdateQueue(snapshotName: string) {
    this.snapshotUpdateQueue.push(snapshotName);
  }

  /**
   * Check if exist snapshot
   */
  public hasSnapshot(snapshotName: string): boolean {
    return this.#currentSnapshots
      ? this.#currentSnapshots.has(snapshotName)
      : false;
  }
}

/**
 * Make an assertion that `actual` matches a snapshot. If the snapshot and `actual` do
 * not a match, then throw.
 *
 * Type parameter can be specified to ensure values under comparison have the same type.
 *
 * @example
 * ```ts
 * import { assertSnapshot } from "https://deno.land/std@$STD_VERSION/testing/snapshot.ts";
 *
 * Deno.test("snapshot", async (test) => {
 *  await assertSnapshot<number>(test, 2);
 * });
 * ```
 */
export async function assertSnapshot<T>(
  context: Deno.TestContext,
  actual: T,
  options: SnapshotOptions<T>,
): Promise<void>;
export async function assertSnapshot<T>(
  context: Deno.TestContext,
  actual: T,
  message?: string,
): Promise<void>;
export async function assertSnapshot(
  context: Deno.TestContext,
  actual: unknown,
  msgOrOpts?: string | SnapshotOptions<unknown>,
) {
  const options = getOptions();
  const assertSnapshotContext = AssertSnapshotContext.fromOptions(
    context,
    options,
  );
  const testName = getTestName(context, options);
  const count = assertSnapshotContext.getCount(testName);
  const name = `${testName} ${count}`;
  const snapshot = await assertSnapshotContext.getSnapshot(
    name,
    options,
  );

  assertSnapshotContext.pushSnapshotToUpdateQueue(name);
  const _serialize = options.serializer || serialize;
  const _actual = _serialize(actual);
  if (getIsUpdate(options)) {
    assertSnapshotContext.registerTeardown();
    if (!equal(_actual, snapshot)) {
      assertSnapshotContext.updateSnapshot(name, _actual);
    }
  } else {
    if (
      !assertSnapshotContext.hasSnapshot(name) ||
      typeof snapshot === "undefined"
    ) {
      throw new AssertionError(
        getErrorMessage(`Missing snapshot: ${name}`, options),
      );
    }
    if (equal(_actual, snapshot)) {
      return;
    }
    let message = "";
    try {
      const usesMultilineDiff = _actual.includes("\n");
      if (usesMultilineDiff) {
        assertEquals(true, false, undefined, {
          formatter: (v) => v ? _actual : snapshot,
        });
      } else {
        assertEquals(_actual, snapshot);
      }
    } catch (e) {
      if (e instanceof AssertionError) {
        message = e.message.replace(
          "Values are not equal.",
          "Snapshot does not match:",
        );
      }
    }
    throw new AssertionError(
      getErrorMessage(message, options),
    );
  }

  function getOptions(): SnapshotOptions {
    if (typeof msgOrOpts === "object" && msgOrOpts !== null) {
      return msgOrOpts;
    }

    return {
      msg: msgOrOpts,
    };
  }
  function getTestName(
    context: Deno.TestContext,
    options?: SnapshotOptions,
  ): string {
    if (options && options.name) {
      return options.name;
    } else if (context.parent) {
      return `${getTestName(context.parent)} > ${context.name}`;
    }
    return context.name;
  }
}

export function createAssertSnapshot<T>(
  options: SnapshotOptions<T>,
  baseAssertSnapshot: typeof assertSnapshot = assertSnapshot,
): typeof assertSnapshot {
  return async function _assertSnapshot(
    context: Deno.TestContext,
    actual: T,
    messageOrOptions?: string | SnapshotOptions<T>,
  ) {
    const mergedOptions: SnapshotOptions<T> = {
      ...options,
      ...(typeof messageOrOptions === "string"
        ? {
          msg: messageOrOptions,
        }
        : messageOrOptions),
    };

    await baseAssertSnapshot(context, actual, mergedOptions);
  };
}
