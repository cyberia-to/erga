# Independent benchmark vectors

Generated before candidate implementation by Go 1.26.3, pinned x/crypto
v0.48.0 BLAKE2b and math/big scalar modulo-2^256 sums. No Erga or native
code imported. Source and command receipts retained in the task audit.

Oracle JSON SHA256 `796f64d55b659a0f547c4dd74ab2307bd60b3795d2a4261d6952c8d9242da613`.
Rows use height 0; V8 odd local nonce indices use height `0x07060504`.
`batches.txt`: rows, kind, wrapping nonce base, count, full XOR digest.
`individual.txt`: rows, kind, nonce, full single digest.
Message is the literal `M_TEST` in support code. These are simplified
benchmark vectors, not Autolykos protocol conformance.
