#include "simple_add.h"

__attribute__((export_name("test_add_1")))
int test_add_1() {
    int a = 1;
    int b = 2;
    int result = add(a, b);
    return result == 3;
}

__attribute__((export_name("test_add_2")))
int test_add_2() {
    // This test should still pass if the addition operator is replaced by a multiplication operator.
    int a = 2;
    int b = 2;
    int result = add(a, b);
    return result == 4;
}

int main() {
    return !(test_add_1() && test_add_2());
}