# Flag Tests

Tests located in sub-directories of this one will be executed by passing the
specified flags to Deno on the command line. Multiple flags are denoted by an
`_` in the directory name.

For example if a test was named `tests/flags/foo/test.ts` with a corresponding
`tests/flags/foo/test.ts.out` the command line to Deno would be:

```
$ deno --foo tests/flags/foo/test.ts
```

If the test was named `tests/flags/foo_bar/test.ts` with a corresponding
`tests/flags/foo_bar/test.ts.out` the command line to Deno would be:

```
$ deno --foo --bar tests/flags/foo_bar/test.ts
```
