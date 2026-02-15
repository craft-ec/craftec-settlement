use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::state::CreatorPool;
use crate::errors::SettlementError;
use crate::events::PoolDeposited;

pub fn handler(ctx: Context<Deposit>, amount: u64) -> Result<()> {
    require!(amount > 0, SettlementError::InvalidAmount);

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.depositor_token_account.to_account_info(),
                to: ctx.accounts.pool_token_account.to_account_info(),
                authority: ctx.accounts.depositor.to_account_info(),
            },
        ),
        amount,
    )?;

    let pool = &mut ctx.accounts.creator_pool;
    pool.pool_balance = pool.pool_balance.checked_add(amount).unwrap();

    emit!(PoolDeposited {
        creator: pool.creator,
        depositor: ctx.accounts.depositor.key(),
        amount,
        new_balance: pool.pool_balance,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub depositor: Signer<'info>,

    #[account(mut)]
    pub creator_pool: Account<'info, CreatorPool>,

    #[account(mut)]
    pub depositor_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = creator_pool,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}
