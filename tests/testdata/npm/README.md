# npm test data

This folder contains test data for npm specifiers.

## Registry

The registry is served by the test server (server in `tests/util/server`) at
http://localhost:4260/ via the `./registry` folder.

### Updating with real npm packages

1. Set the `DENO_TEST_UTIL_UPDATE_NPM=1` environment variable
2. Run the test and it should download the packages.

### Using a custom npm package

1. Add the custom package to `./registry/@denotest`
2. Reference `npm:@denotest/<your-package-name>` in the tests.
