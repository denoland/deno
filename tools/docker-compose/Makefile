# build all the images
build:
	cd deno && docker-compose build

# you can run docker containers with different images layers for debugging and exploration.

run_base:
	cd deno && docker-compose run base

run_prebuild: 
	cd deno && docker-compose run prebuild

run_build: 
	cd deno && docker-compose run build

test:
	cd deno && docker-compose run build /out/Debug/deno meow
	cd deno && docker-compose run build /out/Debug/mock_runtime_test 
