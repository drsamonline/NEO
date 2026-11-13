# Benchmarks

Two benchmarks:

- `benches/fib/` — recursive `fib(35)`: calls, comparisons, integer arithmetic
  and return values.
- `benches/alloc/` — 2000 short-lived array literals with indexing and mutation:
  allocation and bounds-checked array access rather than pure arithmetic.

## Method

`benches/run.sh` does the same thing for each program:

1. `nova-compiler emit <bench>.nova` writes the C for the NOVA program plus the
   runtime (`nova_runtime.c`).
2. Both that C and the hand-written C baseline are compiled with the same
   `cc -O2`.
3. Each binary runs 5 times; the **best** wall-clock time is reported, in
   microseconds, measured with `date +%s%N`.
4. Before timing, the two programs' stdout is compared; the script fails if
   they disagree, and also fails when NOVA→C exceeds `BENCH_MAX_RATIO` (default
   1.5×) of the C baseline.

Run from the repository root:

```sh
benches/run.sh        # 5 runs each
benches/run.sh 20     # more runs
```

## Results (Linux x86-64, best of 5)

```
fib(35): output 9227465 (nova and C agree)
  nova->C      22326 us
  C            21964 us
alloc x2000: output 2005000 (nova and C agree)
  nova->C      2586 us
  C            2591 us
```

Both are within a few percent of the hand-written C baselines on this run. That
is expected: the C backend emits the same shapes, and the allocation benchmark
pays the same `malloc`/runtime cost per array. These numbers are a regression
guard for codegen, not a claim that NOVA is as fast as C in general — neither
benchmark does strings, I/O, or struct-heavy work.

Measured values depend on the machine and load; re-run before comparing.
