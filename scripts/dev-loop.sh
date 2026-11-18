#!/usr/bin/env bash
# Build the compiler on GitHub and run it here.
#
# For a machine with no Rust toolchain: dispatch `dev-build.yml`, wait, download the
# artifact, and point `CC` at whatever C compiler exists locally. The result is a
# compiler built from the branch you asked for, running on this machine.
#
#   ./scripts/dev-loop.sh <branch> [work-dir]
#
# The C compiler is taken from `$CC` if set, otherwise `cc`, otherwise `zig cc` — which
# is how this script was first run, since `pip install ziglang` is the only way to get a
# C compiler into some sandboxes.
set -euo pipefail

branch="${1:?usage: dev-loop.sh <branch> [work-dir]}"
work="${2:-/tmp/nova-dev-loop}"
repo="${NOVA_REPO:-drsamonline/NEO}"

echo "dispatching dev-build.yml on $branch"
gh workflow run dev-build.yml --repo "$repo" --ref "$branch" >/dev/null

# The dispatch is asynchronous and the run id is not returned, so wait for a run on this
# branch that started after we asked.
for _ in $(seq 1 30); do
  run="$(gh run list --repo "$repo" --workflow dev-build.yml --branch "$branch" \
        --limit 1 --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)"
  [ -n "$run" ] && break
  sleep 2
done
[ -n "${run:-}" ] || { echo "no run appeared for $branch" >&2; exit 1; }

echo "run $run — waiting"
gh run watch "$run" --repo "$repo" --interval 15 --exit-status >/dev/null

sha="$(gh run view "$run" --repo "$repo" --json headSha --jq '.headSha')"
echo "downloading nova-dev-$sha"
rm -rf "$work" && mkdir -p "$work"
gh run download "$run" --repo "$repo" -n "nova-dev-$sha" -D "$work"

chmod +x "$work"/nova-* 2>/dev/null || true

if [ -n "${CC:-}" ]; then
  :
elif command -v cc >/dev/null 2>&1; then
  export CC=cc
elif python3 -c 'import ziglang' 2>/dev/null; then
  zig="$(python3 -c 'import ziglang,os;print(os.path.dirname(ziglang.__file__))')/zig"
  printf '#!/bin/sh\nexec %s cc "$@"\n' "$zig" > "$work/cc"
  chmod +x "$work/cc"
  export CC="$work/cc"
else
  echo "no C compiler: set CC, or pip install ziglang" >&2
  exit 1
fi

echo
echo "ready — commit $(cat "$work/COMMIT")"
echo "  compiler: $work/nova-compiler"
echo "  CC:       $CC"
echo "  stdlib:   $work/stdlib"
echo
echo "  $work/nova-compiler build prog.nova -o prog --module-root $work/stdlib"
