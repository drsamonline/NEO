
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int factorial(int n);
int main();

int factorial(int n) {
    if ((n <= 1)) {
        return 1;
    } else {
        return (n * factorial((n - 1)));
    }
}

int main() {
    printf("%s\n", "Factorial of 5:");
    printf("%d\n", factorial(5));
    printf("%s\n", "Counting:");
    for (int i = 0; i < 5; i++) {
        printf("%d\n", i);
    }
    int j = 0;
    while ((j < 3)) {
        printf("%d\n", j);
        j = (j + 1);
    }
    int x = 10;
    if ((x > 5)) {
        printf("%s\n", "x is greater than 5");
    } else {
        printf("%s\n", "x is not greater than 5");
    }
    printf("%f\n", 3.14159);
    printf("%d\n", 1);
    printf("%d\n", 0);
    return 0;
}


