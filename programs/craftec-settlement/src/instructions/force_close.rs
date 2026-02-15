use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::state::{Config, PaymentChannel};
use crate::errors::SettlementError;
use crate::events::{ForceCloseInitiated, ForceCloseFinalised};

/// 2880 slots ≈ 20 minutes at 400ms/slot
pub const FORCE_CLOSE_TIMEOUT_SLOTS: u64 = 2880;

pub fn handler(ctx: Context<ForceClose>) -> Result<()> {
    let channel = &mut ctx.accounts.channel;
    let clock = Clock::get()?;

    if channel.force_close_slot == 0 {
        // Phase 1: initiate
        channel.force_close_slot = clock.slot;
        emit!(ForceCloseInitiated {
            sender: channel.sender,
            receiver: channel.receiver,
            nonce: channel.nonce,
            initiated_slot: clock.slot,
            claimable_slot: clock.slot + FORCE_CLOSE_TIMEOUT_SLOTS,
        });
        return Ok(());
    }

    // Phase 2: finalize
    require!(
        clock.slot >= channel.force_close_slot + FORCE_CLOSE_TIMEOUT_SLOTS,
        SettlementError::ForceCloseTimeout,
    );

    let config = &ctx.accounts.config;
    let spent = channel.voucher_amount;
    let protocol_fee = spent
        .checked_mul(config.protocol_fee_bps as u64).unwrap()
        .checked_div(10_000).unwrap();
    let receiver_payout = spent.checked_sub(protocol_fee).unwrap();
    let sender_refund = channel.locked_amount.checked_sub(spent).unwrap();

    let sender_key = channel.sender;
    let receiver_key = channel.receiver;
    let nonce_bytes = channel.nonce.to_le_bytes();
    let bump = channel.bump;
    let signer_seeds: &[&[u8]] = &[
        b"payment_channel",
        sender_key.as_ref(),
        receiver_key.as_ref(),
        nonce_bytes.as_ref(),
        &[bump],
    ];

    if receiver_payout > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.channel_token_account.to_account_info(),
                    to: ctx.accounts.receiver_token_account.to_account_info(),
                    authority: ctx.accounts.channel.to_account_info(),
                },
                &[signer_seeds],
            ),
            receiver_payout,
        )?;
    }

    if protocol_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.channel_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.channel.to_account_info(),
                },
                &[signer_seeds],
            ),
            protocol_fee,
        )?;
    }

    if sender_refund > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.channel_token_account.to_account_info(),
                    to: ctx.accounts.sender_token_account.to_account_info(),
                    authority: ctx.accounts.channel.to_account_info(),
                },
                &[signer_seeds],
            ),
            sender_refund,
        )?;
    }

    emit!(ForceCloseFinalised {
        sender: sender_key,
        receiver: receiver_key,
        receiver_payout,
        sender_refund,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct ForceClose<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        seeds = [b"payment_channel", channel.sender.as_ref(), channel.receiver.as_ref(), &channel.nonce.to_le_bytes()],
        bump = channel.bump,
        constraint = sender.key() == channel.sender @ SettlementError::NotAuthorized,
    )]
    pub channel: Account<'info, PaymentChannel>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = channel,
    )]
    pub channel_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub receiver_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub sender_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.key() == config.treasury @ SettlementError::NotAuthorized,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}
