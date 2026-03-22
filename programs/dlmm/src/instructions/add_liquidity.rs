use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::error::DlmmErrors;
use crate::{BinArray, Pool, Position, BIN_ARRAY_SEED, POOL_SEED, POSITION_SEED};

#[derive(Accounts)]
#[instruction(lower_bin_id: i32, upper_bin_id: i32)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    // Pool account - holds the pool state, including token mints, vaults, and fees
    #[account(
        mut,
        seeds = [POOL_SEED, token_x.key().as_ref(), token_y.key().as_ref()],
        bump = pool.bump,
        constraint = pool.token_x_mint == token_x.key(),
        constraint = pool.token_y_mint == token_y.key(),
    )]
    pub pool: Account<'info, Pool>,

    // Position account - holds the liquidity shares and fees
    #[account(
        init_if_needed,
        payer = payer,
        space = Position::LEN,
        seeds = [
            POSITION_SEED,
            pool.key().as_ref(),
            payer.key().as_ref(),
            // so that a user can create multiple positions with different bin ranges
            lower_bin_id.to_le_bytes().as_ref(),
            upper_bin_id.to_le_bytes().as_ref(),
        ],
        bump
    )]
    pub position: Account<'info, Position>,

    // Token mints for the pool
    pub token_x: Account<'info, Mint>,
    pub token_y: Account<'info, Mint>,

    // Token vault accounts - hold the pool's token balances
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

    // Payer's token accounts - hold the tokens being added to the pool
    #[account(
        mut,
        constraint = payer_token_x.mint == token_x.key(),
        constraint = payer_token_x.owner == payer.key()
    )]
    pub payer_token_x: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = payer_token_y.mint == token_y.key(),
        constraint = payer_token_y.owner == payer.key()
    )]
    pub payer_token_y: Account<'info, TokenAccount>,

    // Programs - system and token programs
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

/// The selection of bin works in this way:
/// bin_id > pool.active_bin_id => They bin contains X tokens.
/// bin_id == pool.active_bin_id => The both tokens lives here.
/// bin_id < pool.active_bin_id => They bin contains Y tokens
pub fn liquidity_handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, AddLiquidity<'info>>,
    lower_bin_id: i32,
    upper_bin_id: i32,
    amount_x: u64,
    amount_y: u64,
) -> Result<()> {
    // validation checks
    require!(lower_bin_id <= upper_bin_id, DlmmErrors::InvalidBinRange);
    require!(
        upper_bin_id - lower_bin_id < Position::MAX_BINS as i32,
        DlmmErrors::RangeExceedMaxBins
    );
    require!(amount_x > 0 || amount_y > 0, DlmmErrors::ZeroAmount);

    let pool = &ctx.accounts.pool;
    let active_bin_id = pool.active_bin_id;

    // Based on the bin range, validate the token amounts
    if lower_bin_id > active_bin_id {
        require!(amount_y == 0, DlmmErrors::InvalidTokenAmount)
    }
    if upper_bin_id < active_bin_id {
        require!(amount_x == 0, DlmmErrors::InvalidTokenAmount)
    }

    // Bin counts
    let bins_above = if upper_bin_id > active_bin_id {
        (upper_bin_id - active_bin_id) as u64
    } else {
        0
    };

    let bins_below = if active_bin_id > lower_bin_id {
        (active_bin_id - lower_bin_id) as u64
    } else {
        0
    };

    // distributing the amount based on their bins position
    // bin below gets the y tokens, bin above gets the x tokens
    // active bin gets both tokens
    let amount_x_per_bin = if bins_above > 0 {
        amount_x / bins_above
    } else {
        0
    };
    let amount_y_per_bin = if bins_below > 0 {
        amount_y / bins_below
    } else {
        0
    };

    let pool_key = pool.key();
    let program_id = ctx.program_id;

    let position = &mut ctx.accounts.position;
    if position.liquidity_shares.iter().all(|&x| x == 0) {
        position.pool = pool_key;
        position.owner = ctx.accounts.payer.key();
        position.lower_bin_id = lower_bin_id;
        position.upper_bin_id = upper_bin_id;
        position.bump = ctx.bumps.position;
    }

    let mut total_deposited_x: u64 = 0;
    let mut total_deposited_y: u64 = 0;

    let mut remaining_x = amount_x;
    let mut remaining_y = amount_y;

    for bin_id in lower_bin_id..=upper_bin_id {
        // here each slot corresponds to a bin in the range
        let slot = (bin_id - lower_bin_id) as usize;
        require!(slot < Position::MAX_BINS as usize, DlmmErrors::OutOfBounds);

        let (deposit_x, deposit_y) = if bin_id < active_bin_id {
            let y = amount_y_per_bin;
            remaining_y = remaining_y.checked_sub(y).ok_or(DlmmErrors::MathOverflow)?;
            (0u64, y)
        } else if bin_id > active_bin_id {
            let x = amount_x_per_bin;
            remaining_x = remaining_x.checked_sub(x).ok_or(DlmmErrors::MathOverflow)?;
            (x, 0u64)
        } else {
            (remaining_x, remaining_y)
        };

        if deposit_x == 0 && deposit_y == 0 {
            continue;
        }

        // Loading the bin array account details
        let bin_array_loader =
            find_bin_array(ctx.remaining_accounts, &pool_key, bin_id, program_id)?;

        let mut bin_array = bin_array_loader.load_mut()?;
        require!(bin_array.pool == pool_key, DlmmErrors::InvalidBinArray);

        let bin_index = (bin_id - bin_array.start_bin_id) as usize;
        require!(
            bin_index < bin_array.bins.len(),
            DlmmErrors::InvalidBinIndex
        );
        let bin = &mut bin_array.bins[bin_index];

        // shares
        let shares = if bin.total_shares == 0 {
            if deposit_x > 0 && deposit_y > 0 {
                let product = (deposit_x as u128)
                    .checked_mul(deposit_y as u128)
                    .ok_or(DlmmErrors::MathOverflow)?;
                integer_sqrt(product)
            } else {
                (deposit_x as u128)
                    .checked_add(deposit_y as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
            }
        } else {
            let shares_per_x = if bin.reserve_x > 0 && deposit_x > 0 {
                (deposit_x as u128)
                    .checked_mul(bin.total_shares)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.reserve_x as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
            } else {
                0
            };
            let shares_per_y = if bin.reserve_y > 0 && deposit_y > 0 {
                (deposit_y as u128)
                    .checked_mul(bin.total_shares)
                    .ok_or(DlmmErrors::MathOverflow)?
                    .checked_div(bin.reserve_y as u128)
                    .ok_or(DlmmErrors::MathOverflow)?
            } else {
                0
            };

            match (shares_per_x > 0, shares_per_y > 0) {
                (true, true) => shares_per_x.min(shares_per_y),
                (true, false) => shares_per_x,
                (false, true) => shares_per_y,
                _ => return Err(DlmmErrors::ZeroShares.into()),
            }
        };

        require!(shares > 0, DlmmErrors::ZeroShares);

        bin.reserve_x = bin
            .reserve_x
            .checked_add(deposit_x)
            .ok_or(DlmmErrors::MathOverflow)?;

        bin.reserve_y = bin
            .reserve_y
            .checked_add(deposit_y)
            .ok_or(DlmmErrors::MathOverflow)?;

        bin.total_shares = bin
            .total_shares
            .checked_add(shares)
            .ok_or(DlmmErrors::MathOverflow)?;

        let is_fresh_slot = position.liquidity_shares[slot] == 0;

        position.liquidity_shares[slot] = position.liquidity_shares[slot]
            .checked_add(shares)
            .ok_or(DlmmErrors::MathOverflow)?;

        if is_fresh_slot {
            position.fee_growth_inside_x[slot] = bin.fee_growth_x;
            position.fee_growth_inside_y[slot] = bin.fee_growth_y;
        }

        total_deposited_x = total_deposited_x
            .checked_add(deposit_x)
            .ok_or(DlmmErrors::MathOverflow)?;
        total_deposited_y = total_deposited_y
            .checked_add(deposit_y)
            .ok_or(DlmmErrors::MathOverflow)?;
    }

    // Transfers tokens from the payer to the pool's token vault
    if total_deposited_x > 0 {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.payer_token_x.to_account_info(),
                to: ctx.accounts.token_x_vault.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            },
        );
        anchor_spl::token::transfer(cpi_ctx, total_deposited_x)?;
    }

    if total_deposited_y > 0 {
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token::Transfer {
                from: ctx.accounts.payer_token_y.to_account_info(),
                to: ctx.accounts.token_y_vault.to_account_info(),
                authority: ctx.accounts.payer.to_account_info(),
            },
        );
        anchor_spl::token::transfer(cpi_ctx, total_deposited_y)?;
    }

    Ok(())
}

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

pub fn integer_sqrt(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
