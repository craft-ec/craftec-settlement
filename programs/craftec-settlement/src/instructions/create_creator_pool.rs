use anchor_lang::prelude::*;

use crate::state::CreatorPool;
use crate::errors::SettlementError;
use crate::events::CreatorPoolCreated;

pub fn handler(ctx: Context<CreateCreatorPool>, service: u8) -> Result<()> {
    // Data=1, Bundle=4
    require!(service == 1 || service == 4, SettlementError::InvalidService);

    let pool = &mut ctx.accounts.creator_pool;
    pool.creator = ctx.accounts.creator.key();
    pool.service = service;
    pool.pool_balance = 0;
    pool.created_at = Clock::get()?.unix_timestamp;
    pool.current_epoch = 0;
    pool.epoch_balance = 0;
    pool.total_weight = 0;
    pool.distribution_root = [0u8; 32];
    pool.distribution_posted = false;
    pool.last_distribution_at = 0;
    pool.bump = ctx.bumps.creator_pool;

    emit!(CreatorPoolCreated {
        creator: pool.creator,
        service,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CreateCreatorPool<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init,
        payer = creator,
        space = 8 + CreatorPool::INIT_SPACE,
        seeds = [b"creator_pool", creator.key().as_ref()],
        bump,
    )]
    pub creator_pool: Account<'info, CreatorPool>,

    pub system_program: Program<'info, System>,
}
