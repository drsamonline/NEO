=== Generated C Code ===

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int factorial(int n);
void main();

int factorial(int n) {
    if ((n <= 1)) {
        1;
    } else {
        (n * factorial((n - 1)));
    }
}

void main() {
    printf("%d\n", factorial(5));
    for (int i = 0; i < 5; i++) {
        printf("%d\n", i);
    }
    int x = 10;
    while ((x > 0)) {
        printf("%d\n", x);
        x = (x - 1);
    }
    if (1) {
        printf("%d\n", 100);
    } else {
        printf("%d\n", 200);
    }
}


