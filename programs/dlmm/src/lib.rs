pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

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
}
