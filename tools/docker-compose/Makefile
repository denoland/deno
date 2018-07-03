CLI=docker run --rm deno_build

# build all the images
build:
	cd deno && docker-compose build

# you can run docker containers with different images layers for debugging and exploration.

run_base:
	cd deno && docker run --rm -it deno_base

run_prebuild: 
	cd deno && docker run --rm -it deno_prebuild

run_build: 
	cd deno && docker run --rm -it deno_build

test:
	cd deno && $(CLI) out/Debug/mock_runtime_test
	cd deno && $(CLI) out/Debug/deno meow
