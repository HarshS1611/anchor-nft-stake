use anchor_lang::prelude::*;
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    instructions::{
        AddPluginV1CpiBuilder,
        AddCollectionPluginV1CpiBuilder,
        UpdateCollectionPluginV1CpiBuilder,
    },
    types::{
        Attribute, Attributes, FreezeDelegate, Plugin, PluginAuthority, PluginType,
        UpdateAuthority,
    },
    fetch_plugin,
};

use crate::{
    errors::StakingError,
    state::{StakeAccount, StakeConfig, UserAccount},
};

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = asset.owner == owner.key() @ StakingError::NotStaked,
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
        init,
        payer = owner,
        space = 8 + StakeAccount::INIT_SPACE,
        seeds = [b"stake", asset.key().as_ref(), config.key().as_ref()],
        bump,
    )]
    pub stake_account: Box<Account<'info, StakeAccount>>,

    /// CHECK: Verified by address constraint — must equal mpl_core::ID
    #[account(address = mpl_core::ID)]
    pub mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn stake(ctx: Context<Stake>) -> Result<()> {
    // 1. Record stake
    let stake = &mut ctx.accounts.stake_account;
    stake.owner     = ctx.accounts.owner.key();
    stake.mint      = ctx.accounts.asset.key();
    stake.bump      = ctx.bumps.stake_account;
    stake.staked_at = Clock::get()?.unix_timestamp;

    // 2. Increment user counter
    ctx.accounts.user_account.amount_staked =
        ctx.accounts.user_account.amount_staked.saturating_add(1);

    // 3. Add FreezeDelegate to asset — owner signs, stake PDA is delegate
    AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
        .init_authority(PluginAuthority::Address {
            address: ctx.accounts.stake_account.key(),
        })
        .invoke()?;

    // 4. Update collection staked_count — uses collection-specific builders
    update_collection_count(&ctx, 1)?;

    Ok(())
}

fn update_collection_count(ctx: &Context<Stake>, delta: i64) -> Result<()> {
    let collection_info = ctx.accounts.collection.to_account_info();
    let mpl_info        = ctx.accounts.mpl_core_program.to_account_info();
    let owner_info      = ctx.accounts.owner.to_account_info();
    let system_info     = ctx.accounts.system_program.to_account_info();

    // Check if Attributes plugin already exists on the collection
    let plugin_exists = fetch_plugin::<BaseCollectionV1, Attributes>(
        &collection_info,
        PluginType::Attributes,
    ).is_ok();

    if plugin_exists {
        // Use UpdateCollectionPluginV1 — NOT UpdatePluginV1
        // UpdatePluginV1 is for assets only; using it on a collection
        // causes "Error deserializing account"
        let (_, attrs, _) = fetch_plugin::<BaseCollectionV1, Attributes>(
            &collection_info,
            PluginType::Attributes,
        ).unwrap();

        let current: i64 = attrs
            .attribute_list
            .iter()
            .find(|a| a.key == "staked_count")
            .and_then(|a| a.value.parse::<i64>().ok())
            .unwrap_or(0);

        let new_count = (current + delta).max(0).to_string();

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
    } else {
        // Use AddCollectionPluginV1 — NOT AddPluginV1
        // AddPluginV1 is for assets only
        AddCollectionPluginV1CpiBuilder::new(&mpl_info)
            .collection(&collection_info)
            .payer(&owner_info)
            .authority(Some(&owner_info))
            .system_program(&system_info)
            .plugin(Plugin::Attributes(Attributes {
                attribute_list: vec![Attribute {
                    key:   "staked_count".to_string(),
                    value: delta.max(0).to_string(),
                }],
            }))
            .invoke()?;
    }

    Ok(())
}