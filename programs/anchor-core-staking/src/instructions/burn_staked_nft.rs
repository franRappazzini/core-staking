use crate::error::ErrorCode;
use crate::Config;
use anchor_lang::{prelude::*, solana_program::clock::SECONDS_PER_DAY};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{mint_to_checked, Mint, MintToChecked, TokenAccount, TokenInterface},
};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::{
        AddPluginV1CpiBuilder, BurnV1CpiBuilder, UpdateCollectionPluginV1CpiBuilder,
        UpdatePluginV1CpiBuilder,
    },
    types::{
        Attribute, Attributes, BurnDelegate, FreezeDelegate, Plugin, PluginAuthority, PluginType,
        UpdateAuthority,
    },
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct BurnStakedNft<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds = [b"config", collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    #[account(
        mut,
        has_one = owner @ ErrorCode::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub asset: Account<'info, BaseAssetV1>,

    #[account(
        mut,
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority
    )]
    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: This account data is not used, we only verify the address
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"rewards_mint", config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: This is the MPL Core program
    #[account(address = Pubkey::from(MPL_CORE_ID.to_bytes()))]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// 2. Burn-to-Earn with BurnDelegate
/// Create a `burn_staked_nft` instruction that lets users permanently burn their staked NFT for a massive one-time reward bonus.
///
/// Requirements:
/// - Mint reward tokens to the user's ATA
/// - Burn the NFT
pub fn handler(ctx: Context<BurnStakedNft>) -> Result<()> {
    // We start by fetching the existing attributes
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_, attrs, _)| attrs);

    // If the attributes don't exist, we return an error
    require!(attributes_fetched.is_some(), ErrorCode::AssetNotStaked);

    let attributes = attributes_fetched.unwrap();

    // Prepare the Attributes list to update based on the existing attributes
    // let mut attributes_list: Vec<Attribute> = Vec::with_capacity(attributes.attribute_list.len());

    // Additional auxiliary variables
    let current_timestamp = Clock::get()?.unix_timestamp;
    let mut staked_timestamp: i64 = 0;
    let mut staked_time: i64 = 0;

    for attribute in &attributes.attribute_list {
        if attribute.key == "staked" {
            require!(attribute.value == "true", ErrorCode::AssetNotStaked);
        } else if attribute.key == "staked_at" {
            staked_timestamp = staked_timestamp
                .checked_add(
                    attribute
                        .value
                        .parse::<i64>()
                        .map_err(|_| ErrorCode::InvalidTimestamp)?,
                )
                .ok_or(ErrorCode::InvalidTimestamp)?;
            // Calculate the time (in seconds) since the asset was staked
            staked_time = current_timestamp
                .checked_sub(staked_timestamp)
                .ok_or(ErrorCode::InvalidTimestamp)?;
            // Staked time in days
            staked_time = staked_time
                .checked_div(SECONDS_PER_DAY as i64)
                .ok_or(ErrorCode::InvalidTimestamp)?;
            require!(
                staked_time >= ctx.accounts.config.freeze_period as i64,
                ErrorCode::FreezePeriodNotElapsed
            );
        }
    }

    // Prepare signing seeds for the update authority
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // first, we Thaw the asset (update the FreezeDelegate Plugin to false)
    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: false }))
        .invoke_signed(&[signer_seeds])?;

    // then, add burn delegate plugin
    AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::BurnDelegate(BurnDelegate {}))
        .init_authority(PluginAuthority::UpdateAuthority)
        .invoke()?;

    // Now we create the burn cpi
    BurnV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(Some(&ctx.accounts.system_program.to_account_info()))
        .invoke_signed(&[signer_seeds])?;

    // get the collection total_staked attribute to then -1
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseCollectionV1, Attributes>(
        &ctx.accounts.collection.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_, attrs, _)| attrs);

    let mut attribute_list = vec![];

    if let Some(attributes) = &attributes_fetched {
        for attribute in &attributes.attribute_list {
            if attribute.key == "total_staked" {
                let mut total_staked = attribute.value.parse::<u64>().unwrap();
                total_staked = total_staked.checked_sub(1).unwrap();

                attribute_list.push(Attribute {
                    key: "total_staked".to_string(),
                    value: total_staked.to_string(),
                });
            } else {
                attribute_list.push(attribute.clone());
            }
        }
    }

    UpdateCollectionPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .collection(&ctx.accounts.collection.to_account_info())
        .payer(&ctx.accounts.owner.to_account_info())
        .system_program(&ctx.accounts.system_program.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .plugin(mpl_core::types::Plugin::Attributes(Attributes {
            attribute_list,
        }))
        .invoke_signed(&[signer_seeds])?;

    // Finally, we want to mint rewards to the user

    // Calculate the amount
    let amount = (staked_time as u64)
        .checked_mul(ctx.accounts.config.rewards_bps as u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_div(10000u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    let amount = amount
        .checked_add(ctx.accounts.config.burn_rewards) // rewards for burn nft
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    // Prepare signer seeds for config PDA
    let config_seeds = &[
        b"config",
        collection_key.as_ref(),
        &[ctx.accounts.config.bump],
    ];
    let config_signer_seeds = &[&config_seeds[..]];

    mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintToChecked {
                mint: ctx.accounts.rewards_mint.to_account_info(),
                to: ctx.accounts.user_rewards_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            config_signer_seeds,
        ),
        amount,
        ctx.accounts.rewards_mint.decimals,
    )?;

    Ok(())
}
