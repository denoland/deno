# Test coverage

Deno will collect test coverage into a directory for your code if you specify
the `--coverage` flag when starting `deno test`.

This coverage information is acquired directly from the JavaScript engine (V8)
which is very accurate.

This can then be further processed from the internal format into well known
formats by the `deno coverage` tool.

```bash
# Go into your project's working directory
git clone https://github.com/oakserver/oak && cd oak

# Collect your coverage profile with deno test --coverage=<output_directory>
deno test --coverage=cov_profile

# From this you can get a pretty printed diff of uncovered lines
deno coverage cov_profile

# Or generate an lcov report
deno coverage cov_profile --lcov > cov_profile.lcov

# Which can then be further processed by tools like genhtml
genhtml -o cov_profile/html cov_profile.lcov
```

By default, `deno coverage` will exclude any files matching the regular
expression `test\.(js|mjs|ts|jsx|tsx)` and only consider including specifiers
matching the regular expression `^file:` - ie. remote files will be excluded
from coverage report.

These filters can be overridden using the `--exclude` and `--include` flags. A
module specifier must _match_ the include_regular expression and _not match_ the
exclude_ expression for it to be a part of the report.
