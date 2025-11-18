#!/bin/bash
# Environment setup for testing with MinIO

export AWS_ACCESS_KEY_ID=testuser
export AWS_SECRET_ACCESS_KEY=testpass123
export AWS_ENDPOINT_URL=http://localhost:9000
export AWS_DEFAULT_REGION=us-east-1
export MISE_S3_CACHE_BUCKET=mise-cache-test
export MISE_S3_CACHE_REGION=us-east-1
export MISE_S3_CACHE_PREFIX=test-cache
export MISE_S3_CACHE_ENABLED=true

echo "✅ MinIO test environment configured:"
echo "   Endpoint: $AWS_ENDPOINT_URL"
echo "   Bucket: $MISE_S3_CACHE_BUCKET"
echo "   Region: $MISE_S3_CACHE_REGION"