# Benchmarks

The first benchmark is deliberately small and boring: recursive `fib(35)`, which
exercises calls, comparisons, integer arithmetic and return values — the parts
every backend must get right before performance means anything.

## Method

`benches/run.sh` does the same thing for both programs:

1. `nova-compiler emit benches/fib/fib.nova` writes the C for the NOVA program
   plus the runtime (`nova_runtime.c`).
2. Both that C and the hand-written baseline `benches/fib/fib.c` are compiled
   with the same `cc -O2`.
3. Each binary runs 5 times; the **best** wall-clock time is reported, in
   microseconds, measured with `date +%s%N`.
4. Before timing, the two programs' stdout is compared; the script fails if
   they disagree.

Run from the repository root:

```sh
benches/run.sh        # 5 runs each
benches/run.sh 20     # more runs
```

## Result (Linux x86-64, fib(35), best of 5)

```
output: 9227465 (nova and C agree)
nova->C      22551 us
C            22410 us
```

The NOVA→C path is within ~0.6% of the hand-written C baseline on this run. That
is expected: the C backend emits the same recursive shape and the runtime is not
involved in the loop. This number is a regression guard for codegen, not a claim
that NOVA is as fast as C in general — there is one benchmark and it does no
allocation, strings or I/O.

Measured values depend on the machine and load; re-run before comparing.
