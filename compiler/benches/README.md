# No benchmarks live here

This directory is empty on purpose. The benchmarks are in `benches/` at the
repository root:

- `benches/run.sh` — NOVA→C against hand-written C, same `-O2`.
- `benches/compiler.sh` — how long the compiler takes to emit.

Kept as a directory with a signpost because a path that silently does not exist
sends people looking for it.
