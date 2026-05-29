use anchor_lang::prelude::*;
use anchor_spl::token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface};

use crate::{
    errors::StakingError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"config"],
        bump = config.bump,
    )]
    pub config: Box<Account<'info, StakeConfig>>,

    #[account(
        mut,
        seeds = [b"user", owner.key().as_ref()],
        bump = user_account.bump,
    )]
    pub user_account: Box<Account<'info, UserAccount>>,

    #[account(
        mut,
        has_one = owner,
        seeds = [b"stake", stake_account.mint.as_ref(), config.key().as_ref()],
        bump = stake_account.bump,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    #[account(
        mut,
        address = config.rewards_mint,
    )]
    pub rewards_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = rewards_mint,
        token::authority = owner,
        token::token_program = token_program,
    )]
    pub rewards_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
    let clock  = Clock::get()?;
    let stake  = &mut ctx.accounts.stake_account;
    let config = &ctx.accounts.config;

    let elapsed_seconds = clock
        .unix_timestamp
        .checked_sub(stake.staked_at)
        .ok_or(StakingError::RewardOverflow)?;

    let elapsed_days = elapsed_seconds / 86_400;

    require!(elapsed_days > 0, StakingError::NothingToClaim);

    let rewards = (elapsed_days as u64)
        .checked_mul(config.rewards_per_day as u64)
        .ok_or(StakingError::RewardOverflow)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(StakingError::RewardOverflow)?;

    let signer_seeds: &[&[&[u8]]] = &[&[b"config", &[config.bump]]];

    mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintTo {
                mint:      ctx.accounts.rewards_mint.to_account_info(),
                to:        ctx.accounts.rewards_token_account.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            signer_seeds,
        ),
        rewards,
    )?;

    ctx.accounts.user_account.points = ctx
        .accounts
        .user_account
        .points
        .saturating_add(elapsed_days as u32);

    stake.staked_at = clock.unix_timestamp;

    Ok(())
}