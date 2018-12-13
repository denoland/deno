#!/bin/bash
# TODO Port this to python once its working properly.

set -e

cd "$(dirname "$0")"

sam package --template-file template.yaml \
  --output-template-file packaged.yaml --s3-bucket deno.land

sam deploy --template-file packaged.yaml \
  --stack-name denoland4  --capabilities CAPABILITY_IAM

echo "Manually update lambda https://console.aws.amazon.com/lambda/home?region=us-east-1"
