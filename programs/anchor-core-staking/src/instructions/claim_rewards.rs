use crate::error::ErrorCode;
use crate::state::Config;
use anchor_lang::{prelude::*, solana_program::clock::SECONDS_PER_DAY};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface},
};
use mpl_core::{
    accounts::{BaseAssetV1, BaseCollectionV1},
    fetch_plugin,
    instructions::UpdatePluginV1CpiBuilder,
    types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority},
    ID as MPL_CORE_ID,
};

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        seeds    = [b"config", collection.key().as_ref()],
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
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1>,

    /// CHECK: This account is not initialized and is being used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    #[account(
        mut,
        seeds = [b"rewards_mint", config.key().as_ref()],
        bump = config.rewards_bump
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: This is the ID of the MPL Core Program
    #[account(address = MPL_CORE_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// 1. Claim Rewards Without Unstaking
/// Create a `claim_rewards` instruction that lets users collect accumulated rewards without unstaking their NFT.
///
/// Requirements:
/// - Mint reward tokens to the user's ATA
/// - Keep the NFT staked and frozen
pub fn handler(ctx: Context<ClaimRewards>) -> Result<()> {
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
    let mut attribute_list: Vec<Attribute> = Vec::with_capacity(attributes.attribute_list.len());

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
        } else {
            attribute_list.push(attribute.clone());
        }
    }

    // Prepare signing seeds for the update authority
    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority",
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // update the staked_at attribute to now timestamp
    attribute_list.push(Attribute {
        key: "staked_at".to_string(),
        value: Clock::get()?.unix_timestamp.to_string(),
    });

    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .plugin(Plugin::Attributes(Attributes { attribute_list }))
        .invoke_signed(&[signer_seeds])?;

    // Calculate the amount
    let amount = (staked_time as u64)
        .checked_mul(ctx.accounts.config.rewards_bps as u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_div(10000u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    // 2. Mint reward tokens to the user's ATA
    // Prepare signer seeds for config PDA
    let config_seeds = &[
        b"config",
        collection_key.as_ref(),
        &[ctx.accounts.config.bump],
    ];
    let config_signer_seeds = &[&config_seeds[..]];

    anchor_spl::token_interface::mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            anchor_spl::token_interface::MintToChecked {
                mint: ctx.accounts.rewards_mint.to_account_info(),
                to: ctx.accounts.user_rewards_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            config_signer_seeds,
        ),
        amount,
        ctx.accounts.rewards_mint.decimals,
    )
}
