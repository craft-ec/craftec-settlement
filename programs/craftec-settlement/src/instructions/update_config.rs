use anchor_lang::prelude::*;

use crate::state::Config;
use crate::errors::SettlementError;
use crate::events::ConfigUpdated;

pub fn handler(
    ctx: Context<UpdateConfig>,
    new_fee_bps: Option<u16>,
    new_admin: Option<Pubkey>,
    paused: Option<bool>,
) -> Result<()> {
    let config = &mut ctx.accounts.config;

    if let Some(fee) = new_fee_bps {
        require!(fee <= 5000, SettlementError::FeeTooHigh);
        config.protocol_fee_bps = fee;
    }
    if let Some(admin) = new_admin {
        config.admin = admin;
    }
    if let Some(p) = paused {
        config.paused = p;
    }

    emit!(ConfigUpdated {
        protocol_fee_bps: config.protocol_fee_bps,
        admin: config.admin,
        paused: config.paused,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateConfig<'info> {
    #[account(constraint = admin.key() == config.admin @ SettlementError::NotAuthorized)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
}
