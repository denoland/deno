test:
	deno test.ts

fmt:
	prettier *.ts --write

.PHONY: test fmt
