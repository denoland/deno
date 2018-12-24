# Logging module for Deno

Very much work in progress. Contributions welcome.

This library is heavily inspired by Python's
[logging](https://docs.python.org/3/library/logging.html#logging.Logger.log)
module, altough it's not planned to be a direct port. Having separate loggers,
handlers, formatters and filters gives developer very granular control over
logging which is most desirable for server side software.

Todo:

- [ ] implement formatters
- [ ] implement `FileHandler`
- [ ] tests
