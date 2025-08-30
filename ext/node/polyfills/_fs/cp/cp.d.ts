// Copyright 2018-2025 the Deno authors. MIT license.
// Forked from https://github.com/DefinitelyTyped/DefinitelyTyped/blob/0be319a855105188f05c0196a991e2465014244e/types/node/fs.d.ts

interface CopyOptionsBase {
  /**
   * Dereference symlinks
   * @default false
   */
  dereference?: boolean;
  /**
   * When `force` is `false`, and the destination
   * exists, throw an error.
   * @default false
   */
  errorOnExist?: boolean;
  /**
   * Overwrite existing file or directory. _The copy
   * operation will ignore errors if you set this to false and the destination
   * exists. Use the `errorOnExist` option to change this behavior.
   * @default true
   */
  force?: boolean;
  /**
   * Modifiers for copy operation. See `mode` flag of {@link copyFileSync()}
   */
  mode?: number;
  /**
   * When `true` timestamps from `src` will
   * be preserved.
   * @default false
   */
  preserveTimestamps?: boolean;
  /**
   * Copy directories recursively.
   * @default false
   */
  recursive?: boolean;
  /**
   * When true, path resolution for symlinks will be skipped
   * @default false
   */
  verbatimSymlinks?: boolean;
}
export interface CopyOptions extends CopyOptionsBase {
  /**
   * Function to filter copied files/directories. Return
   * `true` to copy the item, `false` to ignore it.
   */
  filter?(source: string, destination: string): boolean | Promise<boolean>;
}
export interface CopySyncOptions extends CopyOptionsBase {
  /**
   * Function to filter copied files/directories. Return
   * `true` to copy the item, `false` to ignore it.
   */
  filter?(source: string, destination: string): boolean;
}
