// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare namespace __bootstrap {
    declare namespace cache {
      declare interface CacheEngine {
        open(cacheNs: string): Promise<void>;
        get(cacheNs: string, key: string): Promise<Response>;
        set(cacheNs: string, key: string, resp: Response): Promise<void>;
        del(cacheNs: string, key: string): Promise<boolean>;
      }
    }
  }
}
