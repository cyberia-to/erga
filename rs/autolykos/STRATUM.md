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

## submit — CONFIRMED accepted

The Bitcoin-style 5-param array (ErgoStratumProxy / ErgoStratumServer),
NOT the 3-param nicehash form. This exact frame was accepted live by
herominers (2026-08-29):

```
→ {"id":101,"method":"mining.submit","params":[
     "<ADDRESS>.erga",       // [0] worker
     "0",                    // [1] jobId
     "00001afa664d",         // [2] extraNonce2 — the searched suffix
     "",                     // [3] nTime (empty for Autolykos)
     "7ad200001afa664d"      // [4] full nonce (keeps the extraNonce1 prefix 7ad2)
  ]}
← {"id":101,"error":null,"result":true}   // ACCEPTED
```

The full nonce keeps the pool's `extraNonce1` as its top bytes; the
`extraNonce2` field is that same nonce with the prefix stripped.

## verified end to end

- chain (`cargo test -p autolykos`): `pow_hit` == sigma-rust's vector.
- GPU (`erga-miner difftest`): kernel hit == the reference, byte-exact,
  and the GPU table build self-checks against the CPU element.
- live: herominers accepted a real share (above) — it earns.
