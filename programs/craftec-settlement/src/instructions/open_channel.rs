use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{self, Mint, Token, TokenAccount, Transfer},
};

use crate::state::{Config, PaymentChannel};
use crate::errors::SettlementError;
use crate::events::ChannelOpened;

pub fn handler(
    ctx: Context<OpenChannel>,
    receiver: Pubkey,
    nonce: u64,
    amount: u64,
) -> Result<()> {
    require!(amount > 0, SettlementError::InvalidAmount);
    require!(!ctx.accounts.config.paused, SettlementError::Paused);

    let channel = &mut ctx.accounts.channel;
    channel.sender = ctx.accounts.sender.key();
    channel.receiver = receiver;
    channel.nonce = nonce;
    channel.locked_amount = amount;
    channel.voucher_amount = 0;
    channel.voucher_seq = 0;
    channel.force_close_slot = 0;
    channel.created_at = Clock::get()?.unix_timestamp;
    channel.bump = ctx.bumps.channel;

    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.sender_token_account.to_account_info(),
                to: ctx.accounts.channel_token_account.to_account_info(),
                authority: ctx.accounts.sender.to_account_info(),
            },
        ),
        amount,
    )?;

    emit!(ChannelOpened {
        sender: channel.sender,
        receiver,
        nonce,
        amount,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(receiver: Pubkey, nonce: u64)]
pub struct OpenChannel<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = sender,
        space = 8 + PaymentChannel::INIT_SPACE,
        seeds = [b"payment_channel", sender.key().as_ref(), receiver.as_ref(), &nonce.to_le_bytes()],
        bump,
    )]
    pub channel: Account<'info, PaymentChannel>,

    #[account(mut)]
    pub sender_token_account: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = sender,
        associated_token::mint = usdc_mint,
        associated_token::authority = channel,
    )]
    pub channel_token_account: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
