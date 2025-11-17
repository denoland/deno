// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
import {
  DatabaseSync,
  op_node_database_backup,
  StatementSync,
} from "ext:core/ops";
import { Buffer } from "node:buffer";

const {
  ObjectDefineProperties,
  SymbolFor,
  SymbolDispose,
} = primordials;

interface BackupOptions {
  /**
   * Name of the source database. This can be `'main'` (the default primary database) or any other
   * database that have been added with [`ATTACH DATABASE`](https://www.sqlite.org/lang_attach.html)
   * @default 'main'
   */
  source?: string | undefined;
  /**
   * Name of the target database. This can be `'main'` (the default primary database) or any other
   * database that have been added with [`ATTACH DATABASE`](https://www.sqlite.org/lang_attach.html)
   * @default 'main'
   */
  target?: string | undefined;
  /**
   * Number of pages to be transmitted in each batch of the backup.
   * @default 100
   */
  rate?: number | undefined;
  /**
   * Callback function that will be called with the number of pages copied and the total number of
   * pages.
   */
  progress?: ((progressInfo: BackupProgressInfo) => void) | undefined;
}

interface BackupProgressInfo {
  totalPages: number;
  remainingPages: number;
}

interface BackupResult {
  totalPages: number;
}

/**
 * This method makes a database backup. This method abstracts the
 * [`sqlite3_backup_init()`](https://www.sqlite.org/c3ref/backup_finish.html#sqlite3backupinit),
 * [`sqlite3_backup_step()`](https://www.sqlite.org/c3ref/backup_finish.html#sqlite3backupstep)
 * and [`sqlite3_backup_finish()`](https://www.sqlite.org/c3ref/backup_finish.html#sqlite3backupfinish) functions.
 *
 * The backed-up database can be used normally during the backup process. Mutations coming from the same connection - same
 * `DatabaseSync` - object will be reflected in the backup right away. However, mutations from other connections will cause
 * the backup process to restart.
 *
 * ```js
 * import { backup, DatabaseSync } from 'node:sqlite';
 *
 * const sourceDb = new DatabaseSync('source.db');
 * const totalPagesTransferred = await backup(sourceDb, 'backup.db', {
 *   rate: 1, // Copy one page at a time.
 *   progress: ({ totalPages, remainingPages }) => {
 *     console.log('Backup in progress', { totalPages, remainingPages });
 *   },
 * });
 *
 * console.log('Backup completed', totalPagesTransferred);
 * ```
 * @param sourceDb The database to backup. The source database must be open.
 * @param path The path where the backup will be created. If the file already exists,
 * the contents will be overwritten.
 * @param options Optional configuration for the backup. The
 * following properties are supported:
 * @returns A promise that resolves when the backup is completed and rejects if an error occurs.
 */
async function backup(
  sourceDb: DatabaseSync,
  path: string | Buffer | URL,
  options?: BackupOptions,
): Promise<number> {
  const result: BackupResult = await op_node_database_backup(
    sourceDb,
    path,
    options,
  );
  return result.totalPages;
}

export const constants = {
  SQLITE_CHANGESET_OMIT: 0,
  SQLITE_CHANGESET_REPLACE: 1,
  SQLITE_CHANGESET_ABORT: 2,

  SQLITE_CHANGESET_DATA: 1,
  SQLITE_CHANGESET_NOTFOUND: 2,
  SQLITE_CHANGESET_CONFLICT: 3,
  SQLITE_CHANGESET_CONSTRAINT: 4,
  SQLITE_CHANGESET_FOREIGN_KEY: 5,
};

const sqliteTypeSymbol = SymbolFor("sqlite-type");
ObjectDefineProperties(DatabaseSync.prototype, {
  [sqliteTypeSymbol]: {
    __proto__: null,
    value: "node:sqlite",
    enumerable: false,
    configurable: true,
  },
  [SymbolDispose]: {
    __proto__: null,
    value: function () {
      try {
        this.close();
      } catch {
        // Ignore errors.
      }
    },
    enumerable: true,
    configurable: true,
    writable: true,
  },
});

export { backup, DatabaseSync, StatementSync };

export default {
  backup,
  constants,
  DatabaseSync,
  StatementSync,
};
