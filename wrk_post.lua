

wrk.method = "POST"

file = io.open("README.md", "rb")
wrk.body = file:read("*a")
wrk.headers["Content-Length"] = tostring(file.size)
