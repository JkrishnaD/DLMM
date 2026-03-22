use anchor_lang::prelude::*;

use crate::{error::DlmmErrors, BinArray};

#[constant]
pub const SEED: &str = "anchor";

pub const POOL_SEED: &[u8] = b"pool";
pub const BIN_ARRAY_SEED: &[u8] = b"bin_array";
pub const POSITION_SEED: &[u8] = b"position";

pub fn find_bin_array<'info>(
    remaining_accounts: &'info [AccountInfo<'info>],
    pool_key: &Pubkey,
    bin_id: i32,
    program_id: &Pubkey,
) -> Result<AccountLoader<'info, BinArray>> {
    // getting the start bin id based on the bin_id as each bin array holds 32 bins
    let start = bin_id.div_euclid(32) * 32;

    // seeds for finding the PDA
    let bindings = start.to_le_bytes();
    let seeds = [BIN_ARRAY_SEED, pool_key.as_ref(), bindings.as_ref()];
    // getting the expected PDA address for the bin array
    let (expected_pda, _) = Pubkey::find_program_address(&seeds, program_id);

    // iterating through the remaining accounts to find the bin array
    for account in remaining_accounts {
        if account.key() == expected_pda {
            return AccountLoader::try_from(account);
        }
    }
    Err(DlmmErrors::BinArrayNotFound.into())
}
