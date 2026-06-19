use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use reed_solomon_erasure::galois_8::ReedSolomon;

/// Constellation FEC parameters: how a proposer's transaction set (a pslice
/// group) gets erasure-coded into pshreds and fanned out to attesters.
pub struct FecParams {
    pub data_shards: usize,
    pub parity_shards: usize,
}

impl FecParams {
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }

    pub fn total(&self) -> usize {
        self.data_shards + self.parity_shards
    }

    pub fn redundancy_ratio(&self) -> f64 {
        self.total() as f64 / self.data_shards as f64
    }
}

/// Split a proposer's payload into `n` equal-length pslices, zero-padding
/// the tail so every slice has identical length (a Reed-Solomon requirement).
pub fn split_into_pslices(payload: &[u8], n: usize) -> (Vec<Vec<u8>>, usize) {
    let shard_len = payload.len().div_ceil(n);
    let mut padded = payload.to_vec();
    padded.resize(shard_len * n, 0);

    let slices = padded.chunks(shard_len).map(<[u8]>::to_vec).collect();
    (slices, shard_len)
}

/// Erasure-code pslices into the full pshred set (data + parity).
pub fn encode_pshreds(
    rs: &ReedSolomon,
    pslices: Vec<Vec<u8>>,
    shard_len: usize,
) -> Result<Vec<Vec<u8>>> {
    let mut shreds = pslices;
    shreds.extend((0..rs.parity_shard_count()).map(|_| vec![0u8; shard_len]));
    rs.encode(&mut shreds).context("encoding pshreds failed")?;
    Ok(shreds)
}

/// Simulate attester packet loss: drop exactly `loss_count` pshreds at
/// random positions, mimicking real, scattered network loss rather than a
/// contiguous block.
pub fn simulate_attester_loss(
    shreds: Vec<Vec<u8>>,
    loss_count: usize,
) -> Vec<Option<Vec<u8>>> {
    let total = shreds.len();
    let mut received: Vec<Option<Vec<u8>>> = shreds.into_iter().map(Some).collect();

    let mut indices: Vec<usize> = (0..total).collect();
    indices.shuffle(&mut rand::thread_rng());

    for &i in indices.iter().take(loss_count) {
        received[i] = None;
    }
    received
}

/// Visualize which pshreds survived ('#') vs were lost ('.').
pub fn survival_map(received: &[Option<Vec<u8>>]) -> String {
    received
        .iter()
        .map(|s| if s.is_some() { '#' } else { '.' })
        .collect()
}

/// Rebuild the full pshred set from whatever arrived, then reassemble the
/// original payload by concatenating the first `data_shards` slices and
/// trimming the zero padding.
pub fn reconstruct_payload(
    rs: &ReedSolomon,
    mut received: Vec<Option<Vec<u8>>>,
    data_shards: usize,
    original_len: usize,
) -> Result<Vec<u8>> {
    rs.reconstruct(&mut received)
        .context("reconstruction failed — too many pshreds lost")?;

    let recovered: Vec<Vec<u8>> = received.into_iter().map(|s| s.unwrap()).collect();
    let mut bytes: Vec<u8> = recovered[..data_shards].concat();
    bytes.truncate(original_len);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRIALS: usize = 100;

    #[test]
    fn round_trip_recovers_from_minimum_shred_count() {
        let payload: &[u8] =
            b"tx1: alice->bob 5 SOL; tx2: carol->dave 1 SOL; tx3: bob->erin 2 SOL";
        let params = FecParams::new(64, 192);

        for trial in 0..TRIALS {
            let rs = ReedSolomon::new(params.data_shards, params.parity_shards).unwrap();

            let (pslices, shard_len) = split_into_pslices(payload, params.data_shards);
            let pshreds = encode_pshreds(&rs, pslices, shard_len).unwrap();
            let received = simulate_attester_loss(pshreds, params.parity_shards);

            let recovered =
                reconstruct_payload(&rs, received, params.data_shards, payload.len())
                    .unwrap_or_else(|e| panic!("trial {trial}: {e}"));

            assert_eq!(recovered, payload, "trial {trial}: mismatch after recovery");
        }
    }
}