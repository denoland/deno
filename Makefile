test:
	cargo build
	cargo test

fmt:
	deno run -A --unstable tools/format.js

lint:
	deno run -A --unstable tools/lint.js

.PHONY: test fmt lint
