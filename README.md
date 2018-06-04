# deno

[![Build Status](https://travis-ci.com/ry/deno.svg?branch=master)](https://travis-ci.com/ry/deno)

A secure TypeScript runtime on V8

* Supports TypeScript 2.8 out of the box. Uses V8 6.8.275.3. That is, it's
  very modern JavaScript.

* No package.json, no npm. Not explicitly compatible with Node.

* Imports reference source code URLs only.
	```
  import { test } from "https://unpkg.com/deno_testing@0.0.5/testing.ts"
  import { log } from "./util.ts"
	```
  Remote code is fetched and cached on first execution, and never updated until
  the code is run with the `--reload` flag. (So this will still work on an
  airplane. See `~/.deno/src` for details on the cache.)

* File system and network access can be controlled in order to run sandboxed
  code. Defaults to read-only file system access and no network access.
	Access between V8 (unprivileged) and Golang (privileged) is only done via
  serialized messages defined in this
  [protobuf](https://github.com/ry/deno/blob/master/msg.proto), this makes it
  easy to audit.
	To enable write access explicitly use `--allow-write` and `--allow-net` for
  network access.

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

* Always dies on uncaught errors.

* Supports top-level await.

* Aims to be browser compatible.

* Can be used as a library to easily build your own JavaScript runtime.
	https://github.com/ry/deno/blob/master/cmd/main.go


## Status

Segfaulty.

No docs yet. For some of the public API see: [deno.d.ts](https://github.com/ry/deno/blob/master/deno.d.ts).

And examples are around here: [testdata/004_set_timeout.ts](https://github.com/ry/deno/blob/master/testdata/004_set_timeout.ts).

Roadmap is [here](https://github.com/ry/deno/blob/master/TODO.txt).

Also see this preentation http://tinyclouds.org/jsconf2018.pdf


## Compile instructions

I will release binaries at some point but for now you have to build it
yourself.

You will need [Go](https://golang.org/) with `$GOPATH` defined and
`$GOPATH/bin` in your `$PATH`.  You will also need
[yarn](https://yarnpkg.com/lang/en/docs/install/) installed.

You need Protobuf 3. On Linux this might work:

``` bash
cd ~
wget https://github.com/google/protobuf/releases/download/v3.1.0/protoc-3.1.0-linux-x86_64.zip
unzip protoc-3.1.0-linux-x86_64.zip
export PATH=$HOME/bin:$PATH
```

On macOS, using [HomeBrew](https://brew.sh/):

``` bash
brew install protobuf
```

Then you need [protoc-gen-go](https://github.com/golang/protobuf/tree/master/protoc-gen-go) and [go-bindata](https://github.com/jteeuwen/go-bindata):

``` bash
go get -u github.com/golang/protobuf/protoc-gen-go
go get -u github.com/jteeuwen/go-bindata/...
```

You need to get and build [v8worker2](https://github.com/ry/v8worker2).  __The package will not build with `go
get` and will log out an error ⚠__
```bash
# pkg-config --cflags v8.pc
Failed to open 'v8.pc': No such file or directory
No package 'v8.pc' found
pkg-config: exit status 1
```

__which can be ignored__. It takes about 30 minutes to build:

``` bash
go get -u github.com/ry/v8worker2
cd $GOPATH/src/github.com/ry/v8worker2
./build.py --use_ccache
```
Maybe also run `git submodule update --init` in the v8worker2 dir.

Finally you can get `deno` and its other Go deps.

``` bash
go get -u github.com/ry/deno/...
```

Now you can build deno and run it:

``` bash
cd $GOPATH/src/github.com/ry/deno

make # Wait for redacted

./deno testdata/001_hello.js # Output: Hello World
```

## make commands

``` bash
make deno # Builds the deno executable.

make test # Runs the tests.

make fmt # Formats the code.

make clean # Cleans the build.
```

