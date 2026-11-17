#!/usr/bin/env bash
# Copy the repository's own programs into the fuzz corpus.
#
# A fuzzer starting from random bytes spends its entire budget failing to produce
# a valid program, so the front-end target never gets past the parser. Seeding
# from the corpus the project already trusts is what makes it find anything.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

targets=(lex parse frontend)
for target in "${targets[@]}"; do
    mkdir -p "fuzz/corpus/$target"
    i=0
    while IFS= read -r program; do
        i=$((i + 1))
        cp "$program" "fuzz/corpus/$target/seed-$i.nova"
    done < <(find examples tests/parity -name '*.nova' | sort)
    echo "seeded fuzz/corpus/$target with $i program(s)"
done
