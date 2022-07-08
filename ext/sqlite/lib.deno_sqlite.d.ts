// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  export namespace sqlite {
    export interface Statement {
      /** Execute a query with provided arguments, without returning
       * anything. */
      run(...arg: any): void;

      /** Execute a query with provided arguments, returning rows. */
      query(...arg: any): any;

      /** Dispose the statement. After calling this function
       * statement is no longer usable.
       */
      close(): void;
    }

    export class Connection {
      /** Open an SQLite database at the specified path. */
      constructor(path: string);

      /** Closes the connection and disposes all statements
            created using this connection.*/
      close(): void;

      /** Returns a statement object that can be used to query
       * the database.
       */
      prepare(sql: string): Statement;
    }
  }
}
