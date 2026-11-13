#!/usr/bin/env bash
# Compile and time the Fibonacci benchmark on the NOVA->C path and a
# hand-written C baseline. Both are built with the same `cc -O2`.
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
"$root/target/debug/nova-compiler" emit benches/fib/fib.nova -o "$out/fib.c" >/dev/null
"$cc_bin" -O2 "$out/fib.c" "$out/nova_runtime.c" -o "$out/fib-nova" -lm
"$cc_bin" -O2 benches/fib/fib.c -o "$out/fib-c"

time_best() {
    local label="$1" binary="$2" best=""
    for _ in $(seq "$runs"); do
        local start end us
        start=$(date +%s%N)
        "$binary" >/dev/null
        end=$(date +%s%N)
        us=$(( (end - start) / 1000 ))
        if [ -z "$best" ] || [ "$us" -lt "$best" ]; then best="$us"; fi
    done
    printf '%-12s %s us (best of %s)\n' "$label" "$best" "$runs"
}

nova_out=$("$out/fib-nova")
c_out=$("$out/fib-c")
if [ "$nova_out" != "$c_out" ]; then
    echo "output mismatch: nova=$nova_out c=$c_out" >&2
    exit 1
fi
echo "output: $nova_out (nova and C agree)"
time_best "nova->C" "$out/fib-nova"
time_best "C" "$out/fib-c"
