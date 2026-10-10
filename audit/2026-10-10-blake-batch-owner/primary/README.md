# Primary provenance

These receipts identify the sources used for the reviewed native import, ordering
and shader-grid proof. Full licensed SDK/web/PDF files remain in temporary caches.
Copied receipt files and input-hash lists preserve original bytes and paths.

- [Initial native acquisition](native-acquisition-initial.json),
  [ordering acquisition](native-sources.json), [SDK paths and hashes](sdk-inputs.json)
  and [review excerpt hashes](native-review-input-hashes.txt): SDK 26.4.1, pinned
  XNU `f6217f891ac0bb64f3d375211650a4c1ff8ca1ea`, Metal no-copy geometry,
  CPU-before-commit visibility and GPU-after-completion ordering. Mach short-info
  ABI: 48 bytes, alignment 4, `is_submap` offset 32, count 12. Runtime leaf-region
  compatibility is a current mapping check; physical/pinning policy stays open.
- [Dispatch acquisition](dispatch-acquisition.json) and
  [review input hashes](dispatch-review-input-hashes.txt): official Apple grid,
  nonuniform threadgroup, family and MSL coordinate evidence. Four unchanged Erga
  shaders use an exact thread grid; no invented device grid query or practical
  allocation limit substitutes for the integer-bound argument.
- [Inline-count acquisition](metal-setbytes-acquisition.json): official Metal
  `setBytes` copies inline bytes. The four-byte count uses existing provider
  encoding; no retained temporary count buffer is needed.
- [Published vector](vector.json): BoringSSL commit
  `09f7078f953362d5c5afdd224118845327b60fb4`, blob
  `8e52cf474275073793a704a2e1509ad131e1847b`. The exact 32-byte input and output
  are recorded, with generation by Go `blake2b.New256(nil)` and the full upstream
  file checksum. The small test fixture retains its license notice.

The [research artifact](../../../.claude/tasks/blake-batch-owner/research.md)
connects these sources to concrete provider and shader paths. Its draft-state
wording is the immutable pre-approval snapshot; execution status is recorded
separately in the task's execution file and this audit.
