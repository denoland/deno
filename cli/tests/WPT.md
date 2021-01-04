## Web Platform Tests

The WPT are test suites for Web platform specs, like Fetch, WHATWG Streams, or console. Deno is able to run most `.any.js` and `.window.js` web platform tests.

This directory contains a `wpt.json` file that is used to configure our WPT test runner. You can use this json file to set which WPT suites to run, and which tests we expect to fail (due to bugs or because they are out of scope for Deno).

To include a new test file to run, add it to the array of test files for the corresponding suite. For example we want to enable `streams/readable-streams/general`. The file would then look like this:

```json
{
  "streams": ["readable-streams/general"]
}
```

If you need more configurability over which test cases in a test file of a suite to run, you can use the object representation. In the example below, we configure `streams/readable-streams/general` to expect `ReadableStream can't be constructed with an invalid type` to fail.

```json
{
  "streams": [
    {
      "name": "readable-streams/general",
      "expectFail": ["ReadableStream can't be constructed with an invalid type"]
    }
  ]
}
```
