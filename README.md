# deno

[![Build Status](https://travis-ci.com/propelml/deno.svg?token=eWz4oGVxypBGsz78gdKp&branch=master)](https://travis-ci.com/propelml/deno)

A JavaScript runtime using V8 6.8 and Go.

* Supports TypeScript 2.8 out of the box.

* No package.json, no npm. Not backwards compatible with Node.

* Imports reference source code URLs only.
	```
  import { test } from "https://unpkg.com/deno_testing@0.0.5/testing.ts"
  import { log } from "./util.ts"
	```

* File system and network access can be controlled in order to run sandboxed
  code. Defaults to read-only file system access. Access between V8
  (unprivileged) and Golang (privileged) is only done via serialized messages
  defined in this protobuf: https://github.com/ry/deno/blob/master/msg.proto
  This makes it easy to audit.

* Single executable:
	```
	> ls -lh deno
	-rwxrwxr-x 1 ryan ryan 55M May 28 23:46 deno
	> ldd deno
		linux-vdso.so.1 =>  (0x00007ffc6797a000)
		libpthread.so.0 => /lib/x86_64-linux-gnu/libpthread.so.0 (0x00007f104fa47000)
		libstdc++.so.6 => /usr/lib/x86_64-linux-gnu/libstdc++.so.6 (0x00007f104f6c5000)
		libm.so.6 => /lib/x86_64-linux-gnu/libm.so.6 (0x00007f104f3bc000)
		libgcc_s.so.1 => /lib/x86_64-linux-gnu/libgcc_s.so.1 (0x00007f104f1a6000)
		libc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x00007f104eddc000)
		/lib64/ld-linux-x86-64.so.2 (0x00007f104fc64000)
	```

* Always dies on on uncaught errors.

* Supports top-level await.

* Aims to be browser compatible.

* Can be used as a library to easily build your own JavaScript runtime.
	https://github.com/ry/deno/blob/master/cmd/main.go


## Status

Segfaulty.

No docs yet. For some of the public API see
https://github.com/ry/deno/blob/master/deno.d.ts

And examples are around here:
https://github.com/ry/deno/blob/master/testdata/004_set_timeout.ts

Roadmap is here: https://github.com/ry/deno/blob/master/TODO.txt


## Compile instructions

I will release binaries at some point but for now you have to build it
yourself.

You need Protobuf 3. On Linux this might work:
```
cd ~
wget https://github.com/google/protobuf/releases/download/v3.1.0/protoc-3.1.0-linux-x86_64.zip
unzip protoc-3.1.0-linux-x86_64.zip
export PATH=$HOME/bin:$PATH
```

Then you need `protoc-gen-go` and `go-bindata` and other deps:
```
go get -u github.com/golang/protobuf/protoc-gen-go
go get -u github.com/jteeuwen/go-bindata/...
go get -u ./...
```

Installing `v8worker2` is time consuming, because it requires building V8. It
may take about 30 minutes:
```
go get -u github.com/ry/v8worker2
cd $GOPATH/src/github.com/ry/v8worker2
./build.py --use_ccache
```

You might also need Node and Yarn.

Finally the dependencies are installed.

Now you can build deno and run it:
```
> make
[redacted]
> ./deno testdata/001_hello.js
Hello World
>
```

## make commands

```bash
make deno # Builds the deno executable

make test # Runs the tests.

make fmt # Formats the code.

make clean # Cleans the build.
```

