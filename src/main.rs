use anyhow::Result;
use constellation_fec::{
    encode_pshreds, reconstruct_payload, simulate_attester_loss, split_into_pslices,
    survival_map, FecParams,
};
use reed_solomon_erasure::galois_8::ReedSolomon;

fn main() -> Result<()> {
    let params = FecParams::new(64, 192);
    let rs = ReedSolomon::new(params.data_shards, params.parity_shards)?;

    let proposer_payload: &[u8] =
        b"tx1: alice->bob 5 SOL; tx2: carol->dave 1 SOL; tx3: bob->erin 2 SOL";

    println!(
        "proposer payload ({} bytes): {:?}\n",
        proposer_payload.len(),
        String::from_utf8_lossy(proposer_payload)
    );
    println!(
        "FEC config: {} data / {} parity ({} total, {:.1}x redundancy)\n",
        params.data_shards,
        params.parity_shards,
        params.total(),
        params.redundancy_ratio()
    );

    let (pslices, shard_len) = split_into_pslices(proposer_payload, params.data_shards);
    let pshreds = encode_pshreds(&rs, pslices, shard_len)?;

    let received = simulate_attester_loss(pshreds, params.parity_shards);
    let held = received.iter().filter(|s| s.is_some()).count();

    println!(
        "fanned out {} pshreds; attester holds {held} (need >= {})",
        params.total(),
        params.data_shards
    );
    println!("\nsurvival map (# held, . lost):\n{}", survival_map(&received));

    let recovered = reconstruct_payload(
        &rs,
        received,
        params.data_shards,
        proposer_payload.len(),
    )?;

    println!("\nsent      : {:?}", String::from_utf8_lossy(proposer_payload));
    println!("recovered : {:?}", String::from_utf8_lossy(&recovered));

    assert_eq!(recovered, proposer_payload, "round-trip mismatch");
    println!(
        "\nrebuilt payload from {} of {} pshreds",
        params.data_shards,
        params.total()
    );

    Ok(())
}