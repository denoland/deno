#!/usr/bin/env bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

USER=$(id -u)
GROUP=$(id -g)

# Use the dev-0.0 tag for local testing
DOCKER_TAG=${DOCKER_TAG:=dev-0.1}
DOCKER_REGISTRY=${DOCKER_REGISTRY:=ry/deno}

DOCKER_IMAGE=${DOCKER_REGISTRY}:${DOCKER_TAG}

docker run --rm -it \
  -u ${USER}:${GROUP} \
  -v ${DIR}:/go/src/github.com/ry/deno \
  -e HOME=/go/src/github.com/ry/deno \
  ${DOCKER_IMAGE} /bin/bash -c "make $1"
