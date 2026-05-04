# PR: Implement Cache.prototype.keys() for Deno's Cache API

## Summary
This PR implements the missing `Cache.prototype.keys()` method for Deno's Cache API. PR #33275 added `CacheStorage.keys()`, but `Cache.keys()` was still undefined, preventing callers from listing cached requests from an opened cache.

## Problem
Currently, when users try to use `cache.keys()` (where `cache` is an instance obtained via `caches.open()`), they get:
```javascript
const cache = await caches.open('test');
console.log(typeof cache.keys); // 'undefined'
await cache.keys(); // TypeError: cache.keys is not a function
```

The `Cache.keys()` method is essential for iterating over cached requests in a specific cache, which is a fundamentally different use case from `CacheStorage.keys()` (which lists cache names).

## Changes

### 1. Rust Backend Changes
- Added `op_cache_keys` operation to the cache extension
- Added `CacheKeysRequest` struct for the operation
- Implemented `keys()` method in `CacheImpl` trait
- Implemented SQLite backend support: queries `request_url` from `request_response_list` table
- Implemented LSC backend stub (returns empty list as LSC doesn't support listing keys directly)

### 2. JavaScript/WebIDL Changes
- Added `op_cache_keys` import in `01_cache.js`
- Implemented `keys()` method in `Cache` class that:
  - Calls the Rust operation to get URLs
  - Converts URLs to `Request` objects (per Web Cache API spec)
  - Returns array of `Request` objects

### 3. TypeScript Definitions
- Added `keys(): Promise<Request[]>` method to `Cache` interface in `lib.deno_cache.d.ts`

### 4. Tests
- Added comprehensive test in `cache_api_test.ts` that:
  - Tests empty cache returns empty array
  - Tests cache with entries returns correct `Request` objects
  - Verifies all expected URLs are present

## API Compatibility
The implementation follows the [Web Cache API specification](https://w3c.github.io/ServiceWorker/#cache-keys):
- Returns a `Promise<Request[]>` 
- Each `Request` in the array represents a cached request
- The order is implementation-defined (currently insertion order from SQLite)

## Testing
The implementation includes:
- Unit test for `Cache.keys()` functionality
- JavaScript syntax validation
- TypeScript definition updates

## Branch Name
`fix-cache-keys-method`

## Commit Message
```
fix(cache): implement missing Cache.keys() method

PR #33275 added CacheStorage.keys() but missed Cache.keys() implementation.
This adds the missing method to allow listing cached requests from an opened cache.

- Add op_cache_keys operation and CacheKeysRequest struct
- Implement keys() method in CacheImpl trait for SQLite and LSC backends
- Add keys() method to Cache class in JavaScript
- Update TypeScript definitions
- Add unit tests for Cache.keys() functionality

Fixes: # (issue number if applicable)
```