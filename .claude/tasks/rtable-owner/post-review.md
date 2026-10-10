# RTable CODE evidence clarification

The final CODE verdict retracts its source findings and says APPROVE, while E1
requests evidence attribution. The frozen execution note already assigns injected
failures to joining and terminal closure; it makes no row-base assertion for them.

[owner.rs](../../../rs/rtable-bench/tests/owner.rs#L32) witnesses row-base arithmetic
through full-row equality using independently enumerated logical indices and the
Rust hash reference, including 65 rows/3 workers. Independent Go checks 16 selected
hash literals and 56 checksum literals separately. The injected callbacks witness
only joining/error closure. The [audit](../../../audit/2026-10-10-rtable-owner/README.md#evidence-boundaries)
now makes that distinction explicit. No source changes or gate reruns were needed;
frozen execution.md, CODE inputs, raw outcomes and capsule remain byte-identical.
