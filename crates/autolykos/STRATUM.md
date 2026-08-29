# Ergo stratum — captured from the wire

Ground truth, captured live from `ergo.herominers.com:1180` (2026-08-29),
not reconstructed from docs. This is what the miner must speak.

## handshake

```
→ {"id":1,"method":"mining.subscribe","params":["erga/0.1.0",null]}
← {"id":1,"error":null,"result":[null,"27ae",6]}
                                        │      │   └ extraNonce2 size (bytes)
                                        │      └ extraNonce1 (hex prefix of the nonce)
                                        └ (subscription details, may be null)
→ {"id":2,"method":"mining.authorize","params":["<ADDRESS>.<worker>","x"]}
← {"id":2,"error":null,"result":true}
```

## job

```
← {"id":null,"method":"mining.set_difficulty","params":[1]}
← {"id":null,"method":"mining.notify","params":[
     "0",                    // [0] jobId
     1861636,                // [1] height        → N = calc_big_n(2, height)
     "eb13e6…79184c6",       // [2] msg           → 32-byte header prehash (hex)
     "",                     // [3] (empty)
     "",                     // [4] (empty)
     2,                      // [5] version
     "2894802230…785380",    // [6] b             → share target as a DECIMAL bigint
     "",                     // [7] (empty)
     true                    // [8] cleanJobs
  ]}
```

- The full 8-byte nonce is `extraNonce1 ++ extraNonce2`: the pool owns the
  `extraNonce1` prefix (`"27ae"` above, 2 bytes), the miner searches the
  remaining `extraNonce2` (6 bytes here) space.
- A share is valid when `pow_hit(msg, nonce, height) < b` (job `[6]`).
  `set_difficulty` adjusts `b` via vardiff on subsequent messages.

## submit

```
→ {"id":N,"method":"mining.submit","params":[
     "<ADDRESS>.<worker>",   // worker
     "0",                    // jobId
     "<extraNonce2 hex>"     // the searched tail; full nonce = extraNonce1 ++ this
  ]}
← {"id":N,"error":null,"result":true}   // accepted
```

## what is verified vs pending

Verified against the chain (`cargo test -p autolykos`): the `pow_hit`
computed here equals sigma-rust's own test vector, and the table-read
search finds nonces that clear a target. So the **hit math a share stands
on is correct**.

Pending live confirmation: this exact notify/submit framing round-trips
to an accepted share. That needs the miner running against the pool long
enough to find one at share difficulty — the final integration step.
