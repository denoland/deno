# Deno Audit Spec Tests

This directory contains spec tests for the `deno audit` command functionality.

## Test Coverage

### Basic Configuration Tests
- **deno_json_only**: Tests audit with only a deno.json file
- **package_json_only**: Tests audit with only a package.json file
- **both_files**: Tests audit with both deno.json and package.json files
- **no_config_files**: Tests audit behavior when no config files are present
- **empty_dependencies**: Tests audit with empty dependency lists

### Flag Tests
- **ignore_unfixable**: Tests the `--ignore-unfixable` flag
- **ignore_registry_errors**: Tests the `--ignore-registry-errors` flag
- **combined_flags**: Tests combining multiple flags together
- **quiet_flag**: Tests the `--quiet` flag

### Level Filter Tests
- **level_low**: Tests `--level low` flag
- **level_moderate**: Tests `--level moderate` flag
- **level_high**: Tests `--level high` flag
- **level_critical**: Tests `--level critical` flag

### Comprehensive Tests
- **comprehensive**: Multiple test scenarios in one directory demonstrating various flag combinations
- **with_vulnerabilities**: Tests output format when vulnerabilities are found (with wildcards for dynamic data)

## Test Structure

Each test directory contains:
- `__test__.jsonc`: Test configuration defining the steps to run
- Input files: `deno.json`, `package.json`, or other necessary files
- Expected output files: `.out` files containing expected command output

## Notes

- Dependencies in the test files are intentionally left empty - they will be filled in when the test npm server implementation is completed
- Output files use `[WILDCARD]` for variable content (like timestamps, package names that may change, etc.)
- Tests are designed to work with the local test npm registry at `http://localhost:4260/`

## Running Tests

These tests are automatically discovered and run by the Deno spec test runner. To run just the audit tests:

```bash
cargo test specs::audit
```
