test:
	deno test.ts

fmt:
	prettier *.md *.ts --write

.PHONY: test fmt
