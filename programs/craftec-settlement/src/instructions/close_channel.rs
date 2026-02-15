use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::state::{Config, PaymentChannel};
use crate::errors::SettlementError;
use crate::events::ChannelClosed;
use crate::voucher::verify_voucher;

pub fn handler(
    ctx: Context<CloseChannel>,
    voucher_amount: u64,
    voucher_seq: u64,
    voucher_signature: [u8; 64],
) -> Result<()> {
    let channel = &ctx.accounts.channel;
    let config = &ctx.accounts.config;

    // Verify ed25519 signature
    let channel_key = ctx.accounts.channel.key();
    require!(
        verify_voucher(
            &channel.sender.to_bytes(),
            &channel_key,
            voucher_amount,
            voucher_seq,
            &voucher_signature,
        ),
        SettlementError::InvalidVoucherSignature,
    );

    require!(voucher_amount <= channel.locked_amount, SettlementError::VoucherExceedsLocked);
    require!(voucher_seq > channel.voucher_seq, SettlementError::StaleVoucher);

    let protocol_fee = voucher_amount
        .checked_mul(config.protocol_fee_bps as u64).unwrap()
        .checked_div(10_000).unwrap();
    let receiver_payout = voucher_amount.checked_sub(protocol_fee).unwrap();
    let sender_refund = channel.locked_amount.checked_sub(voucher_amount).unwrap();

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

    emit!(ChannelClosed {
        sender: sender_key,
        receiver: receiver_key,
        nonce: channel.nonce,
        receiver_payout,
        protocol_fee,
        sender_refund,
    });
    Ok(())
}

#[derive(Accounts)]
pub struct CloseChannel<'info> {
    pub receiver: Signer<'info>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        close = sender,
        seeds = [b"payment_channel", channel.sender.as_ref(), channel.receiver.as_ref(), &channel.nonce.to_le_bytes()],
        bump = channel.bump,
        constraint = receiver.key() == channel.receiver @ SettlementError::NotAuthorized,
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

    /// CHECK: Validated against channel.sender for rent return
    #[account(mut, constraint = sender.key() == channel.sender @ SettlementError::NotAuthorized)]
    pub sender: UncheckedAccount<'info>,

    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}
