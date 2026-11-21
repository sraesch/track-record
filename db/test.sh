#!/usr/bin/env bash
# Runs the pgTAP database tests in a throwaway Docker container.
#
#   db/test.sh                        run all tests
#   db/test.sh /db/tests/05_runs.sql  run selected tests (paths inside the container)
set -euo pipefail

cd "$(dirname "$0")"
image=track-record-db-test

docker build --quiet --tag "$image" test >/dev/null
docker run --rm --user postgres --volume "$PWD:/db:ro" "$image" \
    bash /db/test/run-in-container.sh "$@"
