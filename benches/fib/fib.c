// Hand-written C baseline for benches/fib/fib.nova.
#include <stdio.h>

static int fib(int n) {
    if (n < 2) {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

int main(void) {
    int n = 35;
    printf("%d\n", fib(n));
    return 0;
}
