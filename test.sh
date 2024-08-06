#!/bin/bash -e
SCRIPT_DIR=$(cd $(dirname $0); pwd)
cargo build

TEST_FILE_CONTENT=$(cat "$SCRIPT_DIR/resources/test/test.txt")
TEST_FILE_CONTENT_WITH_PRELOADER=$(LD_PRELOAD="$SCRIPT_DIR/target/debug/libpreloader.so" cat "$SCRIPT_DIR/resources/test/test.txt")

echo "Test 1: Normal file read"
if [ "$TEST_FILE_CONTENT" != "$TEST_FILE_CONTENT_WITH_PRELOADER" ]; then
    echo "Test failed"
    echo "Expected: $TEST_FILE_CONTENT Actual: $TEST_FILE_CONTENT_WITH_PRELOADER"
    exit 1
else
    echo "Test passed"
fi

echo "Test 2: Secret file read"
SECRET_FILE_CONTENT=$(cat "$SCRIPT_DIR/resources/test/secret.txt")
SECRET_FILE_CONTENT_WITH_PRELOADER=$(LD_PRELOAD="$SCRIPT_DIR/target/debug/libpreloader.so" cat "$SCRIPT_DIR/resources/test/secret.txt")
SECRET_FILE_EXPECCTED_CONTENT="Secret!"
if [ "$SECRET_FILE_CONTENT" == "$SECRET_FILE_CONTENT_WITH_PRELOADER" ]; then
    echo "Test failed"
    echo "Expected to get different vaules. Actual: $SECRET_FILE_CONTENT_WITH_PRELOADER"
    exit 1
elif [ "$SECRET_FILE_EXPECCTED_CONTENT" != "$SECRET_FILE_CONTENT_WITH_PRELOADER" ]; then
    echo "Test failed"
    echo "Expected: $SECRET_FILE_EXPECCTED_CONTENT Actual: $SECRET_FILE_CONTENT_WITH_PRELOADER"
    exit 1
else
    echo "Test passed"
fi
