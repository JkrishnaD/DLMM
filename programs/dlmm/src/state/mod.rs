use anchor_lang::prelude::*;

pub const POOL_SEED: &[u8] = b"pool";
pub const BIN_ARRAY_SEED: &[u8] = b"bin_array";
pub const POSITION_SEED: &[u8] = b"position";

#[account]
pub struct Pool {
    pub authority: Pubkey,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub token_x_vault: Pubkey,
    pub token_y_vault: Pubkey,
    pub active_bin_id: i32,
    pub bin_step: u16,
    pub fees: u16,
    pub fee_growth_global_x: u128,
    pub fee_growth_global_y: u128,
    pub bump: u8,
}

impl Pool {
    pub const LEN: usize = 8 + 5 * 32 + 4 + 2 * 2 + 2 * 16 + 1;
}

#[zero_copy]
#[repr(C)]
pub struct Bin {
    pub reserve_x: u64,
    pub reserve_y: u64,
    pub total_shares: u128,
    pub fee_growth_x: u128,
    pub fee_growth_y: u128,
}

impl Bin {
    pub const LEN: usize = 2 * 8 + 3 * 16;
}

#[account(zero_copy)]
#[repr(C)]
pub struct BinArray {
    pub pool: Pubkey,
    pub start_bin_id: i32,
    pub bump: u8,
    pub _padding: [u8; 11],
    pub bins: [Bin; 32],
}

impl BinArray {
    pub const LEN: usize = 8 + 32 + 4 + 1 + 11 + (32 * Bin::LEN);
}

#[account]
pub struct Position {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub lower_bin_id: i32,
    pub upper_bin_id: i32,
    pub fee_owned_by_x: u64,
    pub fee_owned_by_y: u64,
    pub liquidity_share: u128,
    pub fee_growth_inside_x: u128,
    pub fee_growth_inside_y: u128,
    pub bump: u8,
}

impl Position {
    pub const LEN: usize = 8 + 2 * 32 + 2 * 4 + 2 * 8 + 3 * 16 + 1;
}
