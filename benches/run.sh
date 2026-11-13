#!/usr/bin/env bash
# Compile and time the benchmarks on the NOVA->C path and a hand-written C
# baseline. Both are built with the same `cc -O2`.
#
# Usage: benches/run.sh [runs]   (default 5)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="$root/benches/build"
runs="${1:-5}"
cc_bin="${CC:-cc}"

mkdir -p "$out"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"

cargo build -q -p nova-compiler

# --- Fibonacci: calls, comparisons, arithmetic -------------------------------
"$root/target/debug/nova-compiler" emit benches/fib/fib.nova -o "$out/fib.c" >/dev/null
"$cc_bin" -O2 "$out/fib.c" "$out/nova_runtime.c" -o "$out/fib-nova" -lm
"$cc_bin" -O2 benches/fib/fib.c -o "$out/fib-c"

# --- Allocation: array literals, indexing, mutation --------------------------
"$root/target/debug/nova-compiler" emit benches/alloc/alloc.nova -o "$out/alloc.c" >/dev/null
"$cc_bin" -O2 "$out/alloc.c" "$out/nova_runtime.c" -o "$out/alloc-nova" -lm
"$cc_bin" -O2 benches/alloc/alloc.c -o "$out/alloc-c"

time_best() {
    local binary="$1" best=""
    for _ in $(seq "$runs"); do
        local start end us
        start=$(date +%s%N)
        "$binary" >/dev/null
        end=$(date +%s%N)
        us=$(( (end - start) / 1000 ))
        if [ -z "$best" ] || [ "$us" -lt "$best" ]; then best="$us"; fi
    done
    printf '%s' "$best"
}

max_ratio="${BENCH_MAX_RATIO:-1.5}"
failed=0

benchmark() {
    local name="$1" nova_bin="$2" c_bin="$3"
    local nova_out c_out nova_us c_us

    nova_out=$("$nova_bin")
    c_out=$("$c_bin")
    if [ "$nova_out" != "$c_out" ]; then
        echo "$name: output mismatch: nova=$nova_out c=$c_out" >&2
        exit 1
    fi
    echo "$name: output $nova_out (nova and C agree)"
    nova_us=$(time_best "$nova_bin")
    c_us=$(time_best "$c_bin")
    printf '  %-12s %s us (best of %s)\n' "nova->C" "$nova_us" "$runs"
    printf '  %-12s %s us (best of %s)\n' "C" "$c_us" "$runs"

    # Regression guard: the C backend must stay in the same league as
    # hand-written C. BENCH_MAX_RATIO allows some machine noise (default 1.5x).
    if ! awk -v nova="$nova_us" -v c="$c_us" -v max="$max_ratio" \
        'BEGIN { exit !(nova <= c * max) }'; then
        echo "  regression: nova->C ${nova_us}us exceeds ${max_ratio}x C (${c_us}us)" >&2
        failed=1
    fi
}

benchmark "fib(35)" "$out/fib-nova" "$out/fib-c"
benchmark "alloc x2000" "$out/alloc-nova" "$out/alloc-c"

exit "$failed"
