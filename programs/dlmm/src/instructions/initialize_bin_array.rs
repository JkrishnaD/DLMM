use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::{BinArray, Pool, BIN_ARRAY_SEED, POOL_SEED};

#[derive(Accounts)]
#[instruction(start_bin_id: i32)]
pub struct InitializeBinArray<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = BinArray::LEN,
        seeds = [BIN_ARRAY_SEED, pool.key().as_ref(), start_bin_id.to_le_bytes().as_ref()],
        bump
    )]
    pub bin_array: AccountLoader<'info, BinArray>,

    #[account(
        mut,
        seeds = [POOL_SEED, token_x.key().as_ref(), token_y.key().as_ref()],
        bump = pool.bump,
        constraint = pool.token_x_mint == token_x.key(),
        constraint = pool.token_y_mint == token_y.key()
    )]
    pub pool: Account<'info, Pool>,

    pub token_x: Account<'info, Mint>,
    pub token_y: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

pub fn bin_array_handler(ctx: Context<InitializeBinArray>, start_bin_id: i32) -> Result<()> {
    let bin_array = &mut ctx.accounts.bin_array.load_init()?;

    bin_array.start_bin_id = start_bin_id;
    bin_array.pool = ctx.accounts.pool.key();
    bin_array.bump = ctx.bumps.bin_array;

    Ok(())
}
