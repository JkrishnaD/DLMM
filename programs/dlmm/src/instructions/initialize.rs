use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{Pool, POOL_SEED};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>, // User who initializes the pool

    // Pool account initialized by the instruction
    #[account(
        init,
        payer = signer,
        space = Pool::LEN,
        seeds = [POOL_SEED, token_x.key().as_ref(), token_y.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, Pool>,

    // Token vault to store x token
    #[account(
        init,
        payer = signer,
        token::mint = token_x,
        token::authority = signer
    )]
    pub token_x_vault: Account<'info, TokenAccount>,

    // Token vault to store y token
    #[account(
        init,
        payer = signer,
        token::mint = token_y,
        token::authority = signer
    )]
    pub token_y_vault: Account<'info, TokenAccount>,

    // Mint accounts for x and y tokens
    pub token_x: Account<'info, Mint>,
    pub token_y: Account<'info, Mint>,

    // Programs and sysvars required for token operations
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<Initialize>,
    active_bin_id: i32,
    bin_step: u16,
    fees: u16,
) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    // Initialize the pool account with the signer as the authority
    pool.set_inner(Pool {
        authority: ctx.accounts.signer.key(),
        token_x_mint: ctx.accounts.token_x.key(),
        token_y_mint: ctx.accounts.token_y.key(),
        token_x_vault: ctx.accounts.token_x_vault.key(),
        token_y_vault: ctx.accounts.token_y_vault.key(),
        active_bin_id,
        bin_step,
        fees,
        fee_growth_global_x: 0,
        fee_growth_global_y: 0,
        bump: ctx.bumps.pool,
    });
    Ok(())
}
