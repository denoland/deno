wrk.method = "POST"
wrk.headers["Content-Type"] = "application/octet-stream"

file = io.open("./cli/bench/testdata/128k.bin", "rb")
wrk.body = file:read("*a")