#!/usr/bin/env bash
set -euo pipefail

for attempt in 1 2 3; do
    if cargo fetch --locked; then
        # Keep later checks and nested xtask builds independent of the registry.
        echo "CARGO_NET_OFFLINE=true" >> "$GITHUB_ENV"
        exit 0
    fi
    if [ "$attempt" -lt 3 ]; then
        delay=$((attempt * 10))
        echo "::warning::Cargo fetch failed (attempt $attempt/3); retrying in $delay seconds."
        sleep "$delay"
    fi
done

echo "::error::Cargo dependencies could not be fetched after 3 attempts."
exit 1
