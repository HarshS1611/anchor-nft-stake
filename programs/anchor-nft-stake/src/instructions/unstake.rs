use anchor_lang::prelude::*;
use anchor_spl::token_interface::{mint_to, Mint, MintTo, TokenAccount, TokenInterface};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::{
        RemovePluginV1CpiBuilder,
        UpdatePluginV1CpiBuilder,
        UpdateCollectionPluginV1CpiBuilder,
    },
    types::{
        Attribute, Attributes, FreezeDelegate, Plugin, PluginType, UpdateAuthority,
    },
    fetch_plugin,
};

use crate::{
    errors::StakingError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = matches!(
            asset.update_authority,
            UpdateAuthority::Collection(c) if c == config.collection
        ) @ StakingError::InvalidCollection,
    )]
    pub asset: Box<Account<'info, BaseAssetV1>>,

    #[account(
        mut,
        constraint = collection.key() == config.collection
    )]
    pub collection: Box<Account<'info, BaseCollectionV1>>,

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
        seeds = [b"stake", asset.key().as_ref(), config.key().as_ref()],
        bump = stake_account.bump,
        close = owner,
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

    /// CHECK: Verified by address constraint — must equal mpl_core::ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
    let clock  = Clock::get()?;
    let config = &ctx.accounts.config;
    let stake  = &ctx.accounts.stake_account;

    // 1. Enforce freeze period
    let elapsed = clock
        .unix_timestamp
        .checked_sub(stake.staked_at)
        .ok_or(StakingError::RewardOverflow)?;

    require!(
        elapsed >= config.freeze_period as i64,
        StakingError::FreezePeriodNotPassed
    );

    // 2. Auto-claim pending rewards before unstaking
    let elapsed_days = elapsed / 86_400;
    if elapsed_days > 0 {
        let rewards = (elapsed_days as u64)
            .checked_mul(config.rewards_per_day as u64)
            .ok_or(StakingError::RewardOverflow)?
            .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
            .ok_or(StakingError::RewardOverflow)?;

        let config_signer_seeds: &[&[&[u8]]] = &[&[b"config", &[config.bump]]];

        mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                MintTo {
                    mint:      ctx.accounts.rewards_mint.to_account_info(),
                    to:        ctx.accounts.rewards_token_account.to_account_info(),
                    authority: ctx.accounts.config.to_account_info(),
                },
                config_signer_seeds,
            ),
            rewards,
        )?;

        ctx.accounts.user_account.points = ctx
            .accounts
            .user_account
            .points
            .saturating_add(elapsed_days as u32);
    }

    // 3. Thaw: set frozen = false — stake PDA signs as FreezeDelegate authority
    let asset_key  = ctx.accounts.asset.key();
    let config_key = config.key();
    let stake_bump = stake.bump;

    let stake_signer_seeds: &[&[&[u8]]] = &[&[
        b"stake",
        asset_key.as_ref(),
        config_key.as_ref(),
        &[stake_bump],
    ]];

    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.stake_account.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
        .invoke_signed(stake_signer_seeds)?;

    // 4. Remove FreezeDelegate — owner signs (not the stake PDA)
    // After unfreezing, the owner can remove the plugin directly
    // The stake PDA was the delegate authority but owner is the asset authority
    RemovePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin_type(PluginType::FreezeDelegate)
        .invoke()?;

    // 5. Decrement collection staked_count
    update_collection_count(&ctx)?;

    // 6. Decrement user staked counter
    ctx.accounts.user_account.amount_staked =
        ctx.accounts.user_account.amount_staked.saturating_sub(1);

    Ok(())
}

fn update_collection_count(ctx: &Context<Unstake>) -> Result<()> {
    let collection_info = ctx.accounts.collection.to_account_info();
    let mpl_info        = ctx.accounts.mpl_core_program.to_account_info();
    let owner_info      = ctx.accounts.owner.to_account_info();
    let system_info     = ctx.accounts.system_program.to_account_info();

    let current: i64 = if let Ok((_, attrs, _)) =
        fetch_plugin::<BaseCollectionV1, Attributes>(&collection_info, PluginType::Attributes)
    {
        attrs
            .attribute_list
            .iter()
            .find(|a| a.key == "staked_count")
            .and_then(|a| a.value.parse::<i64>().ok())
            .unwrap_or(0)
    } else {
        0
    };

    let new_count = (current - 1).max(0).to_string();

    UpdateCollectionPluginV1CpiBuilder::new(&mpl_info)
        .collection(&collection_info)
        .payer(&owner_info)
        .authority(Some(&owner_info))
        .system_program(&system_info)
        .plugin(Plugin::Attributes(Attributes {
            attribute_list: vec![Attribute {
                key:   "staked_count".to_string(),
                value: new_count,
            }],
        }))
        .invoke()?;

    Ok(())
}