use std::collections::HashMap;

use crate::abi::erc20::events::Transfer;
use substreams::log;
use substreams::scalar::BigInt;
use substreams::Hex;
use substreams_ethereum::pb::eth::v2::{Call, StorageChange};

pub type Address = Vec<u8>;
pub type Hash = Vec<u8>;

pub fn addresses_for_storage_keys<'a>(call: &'a Call) -> HashMap<Hash, Address> {
    let mut out = HashMap::new();

    for (storage_key, preimage) in &call.keccak_preimages {
        // In a standard ERC‑20, each balances[address] entry is stored at
        // keccak256(32‑byte padded address ++ 32‑byte slot), making exactly 64 bytes total.
        // Solidity doesn’t add any extra data, so if you see more than these two 32‑byte words in the mapping preimage,
        // it’s not following the standard ERC‑20 storage layout.
        if preimage.len() != 128 {
            continue;
        }
        if &preimage[64..126] != "00000000000000000000000000000000000000000000000000000000000000" {
            continue;
        }

        // The address is the last 20 bytes of the preimage
        let address: &str = &preimage[24..64];
        out.insert(
            Hex::decode(storage_key).expect("Failed to decode hash hex string"),
            Hex::decode(address).expect("Failed to decode address hex string"),
        );
    }
    out
}

pub fn get_keccak_address<'a>(keccak_address_map: &'a HashMap<Hash, Address>, storage_change: &'a StorageChange) -> Option<Address> {
    let keccak_address = keccak_address_map.get(&storage_change.key);
    match keccak_address {
        Some(address) => Some(address.clone()),
        None => {
            log::info!("storage change does not match any owner address key={}", Hex(&storage_change.key));
            None
        }
    }
}

pub fn is_erc20_valid_address<'a>(address: &Address, transfer: &'a Transfer) -> bool {
    address == &transfer.from || address == &transfer.to
}

pub fn is_erc20_valid_balance<'a>(transfer: &'a Transfer, storage_change: &'a StorageChange) -> bool {
    let old_balance = BigInt::from_signed_bytes_be(&storage_change.old_value);
    let new_balance = BigInt::from_signed_bytes_be(&storage_change.new_value);

    // Absolute balance change
    let balance_change_abs = BigInt::absolute(&(new_balance - old_balance));

    // Absolute transfer value
    let transfer_value_abs = BigInt::absolute(&transfer.value);

    // Compare the difference in absolute terms.
    // We allow a difference of 0 or 1 wei, i.e., if |balance_change_abs - transfer_value_abs| > 1, we continue.
    // https://github.com/streamingfast/substreams-erc20-balance-changes/issues/14
    let diff = BigInt::absolute(&(&balance_change_abs - &transfer_value_abs));
    if diff > BigInt::one() {
        log::info!(
            "Balance change does not match transfer value. Balance change: {}, transfer value: {}",
            balance_change_abs,
            transfer_value_abs
        );
        return false;
    }
    return true;
}

#[cfg(test)]
mod tests {
    use substreams::hex;

    use super::*;

    const NULL_ADDRESS: [u8; 20] = hex!("0000000000000000000000000000000000000000");

    #[test]
    fn test_is_erc20_valid_address() {
        let transfer = Transfer {
            from: NULL_ADDRESS.to_vec(),
            to: hex!("1234567890123456789012345678901234567890").to_vec(),
            value: BigInt::zero(),
        };

        let address = NULL_ADDRESS.to_vec();
        assert!(is_erc20_valid_address(&address, &transfer), "0x000 Null address should be valid");
    }
}
