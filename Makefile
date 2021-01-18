test:
	cargo build -p test_util
	cargo test

fmt:
	deno run -A --unstable tools/format.js

lint:
	deno run -A --unstable tools/lint.js

.PHONY: test fmt lint
