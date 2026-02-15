use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};
use sha2::{Digest, Sha256};

use crate::state::{Config, CreatorPool};
use crate::errors::SettlementError;
use crate::events::RewardsClaimed;

pub fn handler(
    ctx: Context<Claim>,
    operator_pubkey: [u8; 32],
    operator_weight: u64,
    leaf_index: u32,
    merkle_proof: Vec<[u8; 32]>,
) -> Result<()> {
    let pool = &mut ctx.accounts.creator_pool;
    let config = &ctx.accounts.config;

    require!(pool.distribution_posted, SettlementError::DistributionNotPosted);
    require!(pool.total_weight > 0, SettlementError::NoReceipts);

    // Verify Merkle proof
    require!(
        verify_merkle_proof(
            &operator_pubkey,
            operator_weight,
            &merkle_proof,
            leaf_index as usize,
            &pool.distribution_root,
        ),
        SettlementError::InvalidMerkleProof,
    );

    // Calculate payout
    let gross_payout = (operator_weight as u128)
        .checked_mul(pool.epoch_balance as u128)
        .unwrap()
        .checked_div(pool.total_weight as u128)
        .unwrap() as u64;

    let protocol_fee = gross_payout
        .checked_mul(config.protocol_fee_bps as u64)
        .unwrap()
        .checked_div(10_000)
        .unwrap();
    let net_payout = gross_payout.checked_sub(protocol_fee).unwrap();

    require!(gross_payout <= pool.pool_balance, SettlementError::InsufficientPoolBalance);

    // PDA signer seeds
    let creator_key = pool.creator;
    let bump = pool.bump;
    let signer_seeds: &[&[u8]] = &[b"creator_pool", creator_key.as_ref(), &[bump]];

    // Transfer net payout to operator
    if net_payout > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_token_account.to_account_info(),
                    to: ctx.accounts.operator_token_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[signer_seeds],
            ),
            net_payout,
        )?;
    }

    // Transfer fee to treasury
    if protocol_fee > 0 {
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.pool_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: pool.to_account_info(),
                },
                &[signer_seeds],
            ),
            protocol_fee,
        )?;
    }

    pool.pool_balance = pool.pool_balance.checked_sub(gross_payout).unwrap();

    emit!(RewardsClaimed {
        pool: creator_key,
        operator: operator_pubkey,
        gross_payout,
        protocol_fee,
        net_payout,
    });
    Ok(())
}

/// Verify a Merkle proof for a leaf (operator_pubkey, weight).
fn verify_merkle_proof(
    operator_pubkey: &[u8; 32],
    weight: u64,
    proof: &[[u8; 32]],
    index: usize,
    root: &[u8; 32],
) -> bool {
    // Leaf = SHA256(operator_pubkey || weight_le)
    let mut hasher = Sha256::new();
    hasher.update(operator_pubkey);
    hasher.update(weight.to_le_bytes());
    let mut computed: [u8; 32] = hasher.finalize().into();

    let mut idx = index;
    for sibling in proof {
        let mut hasher = Sha256::new();
        if idx % 2 == 0 {
            hasher.update(computed);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(computed);
        }
        computed = hasher.finalize().into();
        idx /= 2;
    }

    computed == *root
}

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"creator_pool", creator_pool.creator.as_ref()],
        bump = creator_pool.bump,
    )]
    pub creator_pool: Account<'info, CreatorPool>,

    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = creator_pool,
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    #[account(mut)]
    pub operator_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = treasury_token_account.key() == config.treasury @ SettlementError::NotAuthorized,
    )]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub usdc_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
}
