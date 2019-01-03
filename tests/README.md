# Integration Tests

This path contains integration tests. When the integration tests are run, the
test harness will execute tests which are defined in a `.test` file and located
in the base of this path.

A `.test` file is a simple configuration format where each option is specified
on a single line. The key is the string to the left of the `:` deliminator and
the value is the string to the right.

| Key         | Required | Description                                                                                                                                                               |
| ----------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `args`      | Yes      | Specifies the command line arguments for the test. This should typically be input script for the test and a `--reload` to help ensure Deno doesn't leverage the cache.    |
| `output`    | Yes      | This is a text file which represents the output of the command. The string `[WILDCARD]` can be used in the output to specify ranges of text which any output is accepted. |
| `exit_code` | No       | If not present, it is assumed the script would exit normally (`0`). If specified, the harness will ensure the proper code is received.                                    |
