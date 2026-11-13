// Hand-written C baseline for benches/alloc/alloc.nova.
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    int total = 0;
    for (int i = 0; i < 2000; i++) {
        int *a = malloc(4 * sizeof(int));
        a[0] = i;
        a[1] = i + 1;
        a[2] = i + 2;
        a[3] = i + 3;
        a[2] = a[2] + 1;
        total = total + a[2];
        free(a);
    }
    printf("%d\n", total);
    return 0;
}
