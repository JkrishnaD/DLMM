use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer};

use crate::{
    error::DlmmErrors, find_bin_array, Pool, Position, FEE_PRECISION, POOL_SEED, POSITION_SEED,
};

#[derive(Accounts)]
#[instruction(lower_bin_id: i32, upper_bin_id: i32)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, token_x.key().as_ref(), token_y.key().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        seeds = [
            POSITION_SEED,
            pool.key().as_ref(),
            owner.key().as_ref(),
            lower_bin_id.to_le_bytes().as_ref(),
            upper_bin_id.to_le_bytes().as_ref(),
        ],
        bump = position.bump,
        constraint = position.owner == owner.key(),
        constraint = position.pool == pool.key(),
    )]
    pub position: Account<'info, Position>,

    pub token_x: Account<'info, Mint>,
    pub token_y: Account<'info, Mint>,

    // pool vaults — source of tokens going back to LP
    #[account(
        mut,
        constraint = token_x_vault.mint == token_x.key(),
        constraint = token_x_vault.owner == pool.key()
    )]
    pub token_x_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = token_y_vault.mint == token_y.key(),
        constraint = token_y_vault.owner == pool.key()
    )]
    pub token_y_vault: Account<'info, TokenAccount>,

    // LP's wallet — destination
    #[account(
        mut,
        constraint = owner_token_x.mint == token_x.key(),
        constraint = owner_token_x.owner == owner.key()
    )]
    pub owner_token_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = owner_token_y.mint == token_y.key(),
        constraint = owner_token_y.owner == owner.key()
    )]
    pub owner_token_y: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn remove_liquidity_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
    lower_bin_id: i32,
    upper_bin_id: i32,
    liquidity_bps: u16,
) -> Result<()> {
    require!(lower_bin_id <= upper_bin_id, DlmmErrors::InvalidBinRange);
    require!(
        upper_bin_id - lower_bin_id < Position::MAX_BINS,
        DlmmErrors::RangeExceedMaxBins
    );
    require!(
        liquidity_bps > 0 && liquidity_bps <= 10_000,
        DlmmErrors::InvalidTokenAmount
    );

    let pool = &ctx.accounts.pool;
    let pool_id = pool.key();
    let program_id = ctx.program_id;

    let position = &mut ctx.accounts.position;

    let mut total_x: u64 = 0;
    let mut total_y: u64 = 0;

    for bin_id in lower_bin_id..=upper_bin_id {
        let slot = (bin_id - lower_bin_id) as usize;
        require!(slot < Position::MAX_BINS as usize, DlmmErrors::OutOfBounds);

        // get the total shares that p
        let user_shares = position.liquidity_shares[slot];
        if user_shares == 0 {
            continue;
        }

        // calculate the shares to remove
        let shares_to_remove = user_shares
            .checked_mul(liquidity_bps as u128)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(10_000)
            .ok_or(DlmmErrors::MathOverflow)?;

        if shares_to_remove == 0 {
            continue;
        }

        // Load the bin array details
        let bin_array_loader =
            find_bin_array(ctx.remaining_accounts, &pool_id, bin_id, program_id)?;
        let mut bin_array = bin_array_loader.load_mut()?;
        require!(bin_array.pool.key() == pool_id, DlmmErrors::InvalidPool);

        let max_bin = bin_array.start_bin_id + bin_array.bins.len() as i32;

        // Bins range validation
        require!(
            bin_id >= bin_array.start_bin_id && bin_id < max_bin,
            DlmmErrors::InvalidBinArray
        );

        let bin_index = (bin_id - bin_array.start_bin_id) as usize;
        require!(
            bin_index < bin_array.bins.len(),
            DlmmErrors::InvalidBinIndex
        );

        let bin = &mut bin_array.bins[bin_index];
        require!(bin.total_shares > 0, DlmmErrors::ZeroShares);

        // calculate the withdraw amounts
        let amount_x = (shares_to_remove)
            .checked_mul(bin.reserve_x as u128)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(bin.total_shares)
            .ok_or(DlmmErrors::MathOverflow)?;

        let amount_y = (shares_to_remove)
            .checked_mul(bin.reserve_y as u128)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(bin.total_shares)
            .ok_or(DlmmErrors::MathOverflow)?;

        // getting the fee growth for the bin
        let fee_growth_x = bin
            .fee_growth_x
            .wrapping_sub(position.fee_growth_inside_x[slot]);

        let fee_growth_y = bin
            .fee_growth_y
            .wrapping_sub(position.fee_growth_inside_y[slot]);

        let fee_x = (fee_growth_x)
            .checked_mul(shares_to_remove)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(FEE_PRECISION as u128)
            .ok_or(DlmmErrors::MathOverflow)? as u64;

        let fee_y = (fee_growth_y)
            .checked_mul(shares_to_remove)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(FEE_PRECISION as u128)
            .ok_or(DlmmErrors::MathOverflow)? as u64;

        position.fee_owned_by_x = position
            .fee_owned_by_x
            .checked_add(fee_x)
            .ok_or(DlmmErrors::MathOverflow)?;

        position.fee_owned_by_y = position
            .fee_owned_by_y
            .checked_add(fee_y)
            .ok_or(DlmmErrors::MathOverflow)?;

        // update the bin reserves
        bin.reserve_x = bin
            .reserve_x
            .checked_sub(amount_x as u64)
            .ok_or(DlmmErrors::MathOverflow)?;
        bin.reserve_y = bin
            .reserve_y
            .checked_sub(amount_y as u64)
            .ok_or(DlmmErrors::MathOverflow)?;
        bin.total_shares = bin
            .total_shares
            .checked_sub(shares_to_remove)
            .ok_or(DlmmErrors::MathOverflow)?;

        // update the position
        position.liquidity_shares[slot] = position.liquidity_shares[slot]
            .checked_sub(shares_to_remove)
            .ok_or(DlmmErrors::MathOverflow)?;

        if position.liquidity_shares[slot] == 0 {
            position.fee_growth_inside_x[slot] = 0;
            position.fee_growth_inside_y[slot] = 0;
        }
        // accumulating the total amounts
        total_x = total_x
            .checked_add(amount_x as u64)
            .ok_or(DlmmErrors::MathOverflow)?;
        total_y = total_y
            .checked_add(amount_y as u64)
            .ok_or(DlmmErrors::MathOverflow)?;
    }

    let token_x_key = ctx.accounts.token_x.key();
    let token_y_key = ctx.accounts.token_y.key();
    let bump = ctx.accounts.pool.bump;

    let seeds = &[
        POOL_SEED,
        token_x_key.as_ref(),
        token_y_key.as_ref(),
        &[bump],
    ];

    let signer_seeds = &[&seeds[..]];

    let fully_exited = position.liquidity_shares.iter().all(|s| *s == 0);
    if fully_exited {
        let lamports = position.get_lamports();
        position.sub_lamports(lamports)?;
        ctx.accounts.owner.add_lamports(lamports)?;
    }

    // transfer the tokens
    if total_x > 0 {
        let accounts = Transfer {
            from: ctx.accounts.token_x_vault.to_account_info(),
            to: ctx.accounts.owner_token_x.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        anchor_spl::token::transfer(cpi_ctx, total_x)?;
    }

    if total_y > 0 {
        let accounts = Transfer {
            from: ctx.accounts.token_y_vault.to_account_info(),
            to: ctx.accounts.owner_token_y.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            accounts,
            signer_seeds,
        );

        anchor_spl::token::transfer(cpi_ctx, total_y)?;
    }

    Ok(())
}
