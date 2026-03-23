use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{error::DlmmErrors, find_bin_array, Pool, FEE_PRECISION, POOL_SEED};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, token_x.key().as_ref(), token_y.key().as_ref()],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    // token mints
    pub token_x: Account<'info, Mint>,
    pub token_y: Account<'info, Mint>,

    // pool token vaults
    #[account(
        mut,
        constraint = token_x_vault.mint == token_x.key(),
        constraint = token_x_vault.owner == pool.key(),
    )]
    pub token_x_vault: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = token_y_vault.mint == token_y.key(),
        constraint = token_y_vault.owner == pool.key(),
    )]
    pub token_y_vault: Account<'info, TokenAccount>,

    // user token accounts
    #[account(
        mut,
        constraint = user_token_x.mint == token_x.key(),
        constraint = user_token_x.owner == user.key(),
    )]
    pub user_token_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = user_token_y.mint == token_y.key(),
        constraint = user_token_y.owner == user.key(),
    )]
    pub user_token_y: Account<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn swap_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, Swap>,
    amount_in: u64,
    min_amount_out: u64,
    swap_for_y: bool,
) -> Result<()> {
    require!(amount_in > 0, DlmmErrors::InvalidAmount);

    // getting the pool account details
    let pool = &mut ctx.accounts.pool;
    let pool_key = pool.key();
    let program_id = ctx.program_id;

    let mut current_bin_id = pool.active_bin_id;

    let mut remaining_input = amount_in as u128;
    let mut total_output: u128 = 0;

    let mut steps = 0;

    // loop where swaps are processed if we give valid input amount
    while remaining_input > 0 {
        // break if we exceed 35 steps to avoid infinite loops
        // which means we have only 35 bins to search through
        steps += 1;
        require!(steps < 35, DlmmErrors::OutOfLiquidity);

        // getting the bin array details
        let bin_array_loader = find_bin_array(
            ctx.remaining_accounts,
            &pool_key,
            current_bin_id,
            program_id,
        )?;
        let mut bin_array = bin_array_loader.load_mut()?;
        require!(bin_array.pool == pool_key, DlmmErrors::InvalidBinArray);

        let bin_index = (current_bin_id - bin_array.start_bin_id) as usize;
        require!(
            bin_index < bin_array.bins.len(),
            DlmmErrors::InvalidBinIndex
        );

        let bin = &mut bin_array.bins[bin_index];

        // if the bin reservers are empty, then we are moving to the next bin
        if bin.reserve_x == 0 || bin.reserve_y == 0 {
            current_bin_id = if swap_for_y {
                current_bin_id + 1
            } else {
                current_bin_id - 1
            };
            continue;
        }

        // fee accumulation logic
        let fee = remaining_input
            .checked_mul(pool.fees as u128)
            .ok_or(DlmmErrors::MathOverflow)?
            .checked_div(10_000)
            .ok_or(DlmmErrors::MathOverflow)?;

        let effective_input = remaining_input
            .checked_sub(fee)
            .ok_or(DlmmErrors::MathOverflow)?;

        // AMM math
        let amount_out = if swap_for_y {
            effective_input
                .checked_mul(bin.reserve_y as u128)
                .ok_or(DlmmErrors::MathOverflow)?
                .checked_div(
                    (bin.reserve_x as u128)
                        .checked_add(effective_input)
                        .ok_or(DlmmErrors::MathOverflow)?,
                )
                .ok_or(DlmmErrors::MathOverflow)?
        } else {
            effective_input
                .checked_mul(bin.reserve_x as u128)
                .ok_or(DlmmErrors::MathOverflow)?
                .checked_div(
                    (bin.reserve_y as u128)
                        .checked_add(effective_input)
                        .ok_or(DlmmErrors::MathOverflow)?,
                )
                .ok_or(DlmmErrors::MathOverflow)?
        };

        let available_output = if swap_for_y {
            bin.reserve_y as u128
        } else {
            bin.reserve_x as u128
        };

        // CASE 1: bin has enough liquidity
        if amount_out <= available_output {
            // update reserves
            if swap_for_y {
                bin.reserve_x += effective_input as u64;
                bin.reserve_y -= amount_out as u64;
            } else {
                bin.reserve_y += effective_input as u64;
                bin.reserve_x -= amount_out as u64;
            }

            // fee growth
            if bin.total_shares > 0 {
                let fee_per_share = fee
                    .checked_mul(FEE_PRECISION as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.total_shares)
                    .ok_or(DlmmErrors::MathOverflow)?;

                if swap_for_y {
                    bin.fee_growth_x += fee_per_share;
                } else {
                    bin.fee_growth_y += fee_per_share;
                }
            }

            total_output += amount_out;
            break;
        }
        // CASE 2: consume full bin
        else {
            let amount_out = available_output;

            let amount_in_used = if swap_for_y {
                (amount_out)
                    .checked_mul(bin.reserve_x as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.reserve_y as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
            } else {
                (amount_out)
                    .checked_mul(bin.reserve_y as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.reserve_x as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
            };

            let fee = amount_in_used
                .checked_mul(pool.fees as u128)
                .ok_or(DlmmErrors::MathOverflow)?
                .checked_div(10_000)
                .ok_or(DlmmErrors::MathOverflow)?;

            let effective_input = amount_in_used
                .checked_sub(fee)
                .ok_or(DlmmErrors::MathOverflow)?;

            // update reserves
            if swap_for_y {
                bin.reserve_x += effective_input as u64;
                bin.reserve_y = 0;
            } else {
                bin.reserve_y += effective_input as u64;
                bin.reserve_x = 0;
            }

            // fee growth
            if bin.total_shares > 0 {
                let fee_per_share = fee
                    .checked_mul(FEE_PRECISION as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.total_shares)
                    .ok_or(DlmmErrors::MathOverflow)?;

                if swap_for_y {
                    bin.fee_growth_x += fee_per_share;
                } else {
                    bin.fee_growth_y += fee_per_share;
                }
            }

            total_output += amount_out;
            remaining_input -= amount_in_used;

            // move to next bin
            current_bin_id = if swap_for_y {
                current_bin_id + 1
            } else {
                current_bin_id - 1
            };
        }
    }

    // slippage check
    require!(
        total_output >= min_amount_out as u128,
        DlmmErrors::SlippageExceeded
    );

    pool.active_bin_id = current_bin_id;

    // getting the token vaults based on the swap direction
    let (token_in_vault, token_out_vault) = if swap_for_y {
        (&ctx.accounts.token_x_vault, &ctx.accounts.token_y_vault)
    } else {
        (&ctx.accounts.token_y_vault, &ctx.accounts.token_x_vault)
    };

    // user → vault transfer
    let cpi_ctx = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        anchor_spl::token::Transfer {
            from: if swap_for_y {
                ctx.accounts.user_token_x.to_account_info()
            } else {
                ctx.accounts.user_token_y.to_account_info()
            },
            to: token_in_vault.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        },
    );
    anchor_spl::token::transfer(cpi_ctx, amount_in)?;

    // vault → user transfer
    let token_x_key = ctx.accounts.token_x.key();
    let token_y_key = ctx.accounts.token_y.key();

    let seeds = &[
        POOL_SEED,
        token_x_key.as_ref(),
        token_y_key.as_ref(),
        &[pool.bump],
    ];
    let signer = &[&seeds[..]];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        anchor_spl::token::Transfer {
            from: token_out_vault.to_account_info(),
            to: if swap_for_y {
                ctx.accounts.user_token_y.to_account_info()
            } else {
                ctx.accounts.user_token_x.to_account_info()
            },
            authority: ctx.accounts.pool.to_account_info(),
        },
        signer,
    );
    anchor_spl::token::transfer(cpi_ctx, total_output as u64)?;

    Ok(())
}
