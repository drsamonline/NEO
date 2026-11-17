#!/usr/bin/env bash
# How long the compiler takes to emit, not how fast the code it emits runs.
#
# `benches/run.sh` answers "is the generated C fast?". Nothing answered "is the
# compiler fast?", which is the number a user feels on every keystroke and every
# `cargo build` of a NOVA project. This measures the front end and the C
# generator over the whole corpus.
#
# It is a trend, not a gate: wall-clock on a shared runner is noisy, so nothing
# here fails a build. `bench.yml` records the number weekly and the run log is
# the record.
#
# Usage: benches/compiler.sh [runs]   (default 5, best-of is reported)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runs="${1:-5}"
out="$root/benches/build/compiler"
mkdir -p "$out"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"

cargo build -q --release -p nova-compiler
nova="$root/target/release/nova-compiler"

mapfile -t programs < <(find examples tests/parity -name '*.nova' | sort)
if [ "${#programs[@]}" -eq 0 ]; then
    echo "no programs found under examples/ or tests/parity/" >&2
    exit 1
fi

# One pass over the corpus, timed.
run_once() {
    local start end
    start=$(date +%s%N)
    for program in "${programs[@]}"; do
        "$nova" emit "$program" -o "$out/$(basename "$program" .nova).c" >/dev/null
    done
    end=$(date +%s%N)
    echo $(( (end - start) / 1000000 ))
}

echo "corpus: ${#programs[@]} programs, ${runs} runs, best of"
best=""
total=0
for _ in $(seq "$runs"); do
    ms=$(run_once)
    total=$(( total + ms ))
    if [ -z "$best" ] || [ "$ms" -lt "$best" ]; then best="$ms"; fi
done

mean=$(( total / runs ))
echo "best: ${best} ms"
echo "mean: ${mean} ms"
echo "per program (best): $(( best / ${#programs[@]} )) ms"

# The number that matters is the trend across weeks. A single run tells you
# whether the machine was busy, and nothing else.
