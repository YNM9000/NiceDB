#!/usr/bin/env bash
set -euo pipefail

args=()
join=""
for a in "$@"; do
    if [[ -n "$join" ]]; then
        args+=("-Wl,-force_load,${a#-Wl,}")
        join=""
    elif [[ "$a" == "-Wl,-force_load" ]]; then
        join=1
    else
        args+=("$a")
    fi
done

exec cargo-zigbuild zig cc -- -g -fno-sanitize=all -target "${ND_ZIG_TARGET:?ND_ZIG_TARGET must be set}" "${args[@]}"
