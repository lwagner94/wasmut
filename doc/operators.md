
# Supported Mutation operators

The mutation operators available in `wasmut` are for now mainly based on [mull's operators](https://mull.readthedocs.io/en/latest/SupportedMutations.html)

| Name                        | Description                                                                  |
| ---                         | ---                                                                          |
| `binop_sub_to_add`          | Replace subtraction with addition                                            |
| `binop_add_to_sub`          | Replace addition with subtraction                                            |
| `binop_mul_to_div`          | Replace multiplication with signed/unsigned division                         |
| `binop_div_to_mul`          | Replace signed/unsigned division by multiplication                           |
| `binop_shl_to_shr`          | Replace bitwise left-shift with signed/unsigned right-shift                  |
| `binop_shr_to_shl`          | Replace signed/unsigned right-shift with left-shift                          |
| `binop_rem_to_div`          | Replace remainder with  division of the same signedness                      |
| `binop_div_to_rem`          | Replace division with remainder of the same signedness                       |
| `binop_and_to_or`           | Replace and with or                                                          |
| `binop_or_to_and`           | Replace or with and                                                          |
| `binop_xor_to_or`           | Replace xor with or                                                          | 
| `binop_or_to_xor`           | Replace or with xor                                                          |
| `binop_rotl_to_rotr`        | Replace bitwise left-rotation with right-rotation                            |
| `binop_rotr_to_rotl`        | Replace bitwise right-rotation with left-rotation                            |
| `unop_neg_to_nop`           | Replace unary negation with nop                                              |
| `relop_eq_to_ne`            | Replace equality test with not-equal                                         |
| `relop_ne_to_eq`            | Replace not-equal test with equality                                         |
| `relop_le_to_gt`            | Replace less-equal with greater-than of the same signedness                  |
| `relop_le_to_lt`            | Replace less-equal with less-than of the same signedness                     | 
| `relop_lt_to_ge`            | Replace less-than with greater-equal of the same signedness                  |
| `relop_lt_to_le`            | Replace less-than with less-equal of the same signedness                     |
| `relop_ge_to_gt`            | Replace greater-equal with greater-than of the same signedness               |
| `relop_ge_to_lt`            | Replace greater-than with less-than of the same signedness                   |
| `relop_gt_to_ge`            | Replace greater-than with greater-equal of the same signedness               |
| `relop_gt_to_le`            | Replace greater-than with less-equal of the same signedness                  |
| `const_replace_zero`        | Replace zero constants with 42                                               |
| `const_replace_nonzero`     | Replace non-zero constants with 0                                            |
| `call_remove_void_call`     | Remove calls to functions that do not have a return value                    |
| `call_remove_scalar_call`   | Remove calls to functions that return a single scalar with the value of 42   |

