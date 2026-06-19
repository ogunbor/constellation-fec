# constellation-fec

A Reed-Solomon erasure coding harness modeling how a Constellation (MCP)
proposer spreads a pslice across attesters as pshreds.

## Background

Constellation is Anza's Multiple Concurrent Proposers (MCP) proposal for
Solana, designed to run alongside Alpenglow. Under Constellation, each
proposer erasure-codes its transaction set into **pslices** and distributes
them as **pshreds** to a set of attesters during each 50ms cycle. If enough
attesters observe shreds for a proposal, it must be included in the next
execution cycle — censorship resistance enforced cryptographically rather
than by policy.

This terminology is deliberately distinct from Alpenglow's "slices" and
"shreds" to formally separate proposer-level erasure coding from
leader-level block propagation (Turbine).

This project reuses the same `reed_solomon_erasure` crate that powers
Solana's existing Turbine shredding (`ledger/src/shredder.rs`), but
parameterised for Constellation's pshred/pslice model.

## What it does

A proposer's transaction list (the payload) is split into `data_shards`
equal-length pslices, then erasure-coded into `data_shards + parity_shards`
total pshreds. Any `data_shards` of the resulting pshreds — regardless of
which ones — are sufficient to reconstruct the original payload exactly.

The default configuration uses 64 data shards and 192 parity shards
(256 total, 4x redundancy): up to three-quarters of all pshreds can be lost
and the original payload still recovers in full.

## Project structure

```
constellation-fec/
├── Cargo.toml
└── src/
    ├── lib.rs     # FecParams + reusable encode/decode/loss-simulation functions
    └── main.rs     # demo driver — encodes, simulates loss, reconstructs, verifies
```

### `lib.rs`

| Function | Purpose |
|---|---|
| `FecParams::new(data, parity)` | Defines the FEC shape: shard counts, total, redundancy ratio |
| `split_into_pslices` | Splits a payload into equal-length, zero-padded pslices |
| `encode_pshreds` | Reed-Solomon encodes pslices into the full pshred set |
| `simulate_attester_loss` | Randomly drops a configurable number of pshreds |
| `survival_map` | Visualizes which pshreds survived (`#`) vs were lost (`.`) |
| `reconstruct_payload` | Rebuilds the original payload from surviving pshreds |

### `main.rs`

A runnable demo that wires the library functions together: encodes a sample
transaction list, simulates attester loss at the maximum tolerable rate,
reconstructs, and asserts the recovered bytes match the original exactly.

## Running it

```bash
cargo run
```

Prints the payload, the FEC configuration, a survival map of which pshreds
were held vs lost, and the recovered payload — confirming it matches the
original.

## Testing

```bash
cargo test --release
```

Runs 100 trials, each dropping a different random set of pshreds (up to the
maximum tolerable loss), and checks the payload always recovers intact
regardless of which specific shreds survive.

> Use `--release`. The Galois field arithmetic underlying Reed-Solomon is
> significantly slower in debug builds.

## Why round-tripping is the correctness test

Round-tripping — encoding, simulating loss, decoding, and verifying the
recovered bytes match the original — is the only test that actually proves
Reed-Solomon correctness here. It is not enough to check that `encode()` or
`reconstruct()` return `Ok(())`; the only guarantee that matters is that the
**bytes you get back are identical to the bytes you sent**, no matter which
subset of pshreds survived.

## Dependencies

- [`reed-solomon-erasure`](https://crates.io/crates/reed-solomon-erasure) — the erasure coding implementation, with `simd-accel` enabled
- [`rand`](https://crates.io/crates/rand) — randomized shred-loss simulation
- [`anyhow`](https://crates.io/crates/anyhow) — error context and propagation

## Status / next steps

- [ ] Derive `FecParams` from realistic Constellation cycle parameters
      (payload size, cycle duration, attester count) instead of a fixed
      64/192 split, mirroring `ReedSolomonCache` and FEC set sizing logic
      in `ledger/src/shredder.rs`
- [ ] Replace in-process random loss simulation with real network-level
      loss via `tc netem`
- [ ] Extend benchmark coverage (see [agave#5695](https://github.com/anza-xyz/agave/pull/5695))
      to include the new Constellation-specific parameters