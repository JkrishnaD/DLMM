use anchor_lang::prelude::*;

#[account]
pub struct Pool {
    pub authority: Pubkey,
    pub token_x_mint: Pubkey,
    pub token_y_mint: Pubkey,
    pub total_bins: u32,
    pub active_bin_id: i32,
    pub bin_step: u16,
    pub fees: u16,
    pub bump: u8,
}

impl Pool {
    pub const LEN: usize = 8 + 3 * 32 + 4 * 2 + 2 * 2 + 1;
}

#[zero_copy]
#[repr(C)]
pub struct Bin {
    pub reserve_x: u64,
    pub reserve_y: u64,
    pub fee_x: u64,
    pub fee_y: u64,
    pub price: u64,
    pub _padding: [u8; 8],
}

impl Bin {
    pub const LEN: usize = 5 * 8 + 8;
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
    pub liquidity_x: u64,
    pub liquidity_y: u64,
    pub bin_id: u16,
    pub bump: u8,
}

impl Position {
    pub const LEN: usize = 8 + 2 * 32 + 2 * 8 + 2 + 1;
}
