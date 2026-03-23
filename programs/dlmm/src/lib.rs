use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("J7efo9smyWU6SXPajaibUMNLW7DZKFq5nFDpa2KttXj");

#[program]
pub mod dlmm {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        active_bin_id: i32,
        bin_step: u16,
        fees: u16,
    ) -> Result<()> {
        initialize::handler(ctx, active_bin_id, bin_step, fees)
    }

    pub fn initialize_bin_array(ctx: Context<InitializeBinArray>, start_bin_id: i32) -> Result<()> {
        initialize_bin_array::bin_array_handler(ctx, start_bin_id)
    }

    pub fn add_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, AddLiquidity<'info>>,
        lower_bin_id: i32,
        upper_bin_id: i32,
        amount_x: u64,
        amount_y: u64,
    ) -> Result<()> {
        add_liquidity::add_liquidity_handler(ctx, lower_bin_id, upper_bin_id, amount_x, amount_y)
    }

    pub fn remove_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
        lower_bin_id: i32,
        upper_bin_id: i32,
        liquidity_bps: u16,
    ) -> Result<()> {
        remove_liquidity::remove_liquidity_handler(ctx, lower_bin_id, upper_bin_id, liquidity_bps)
    }

    pub fn remove_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
        lower_bin_id: i32,
        upper_bin_id: i32,
        liquidity_bps: u16,
    ) -> Result<()> {
        remove_liquidity::remove_liquidity_handler(ctx, lower_bin_id, upper_bin_id, liquidity_bps)
    }
}
