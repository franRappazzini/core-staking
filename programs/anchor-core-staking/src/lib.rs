#![allow(unexpected_cfgs, deprecated, ambiguous_glob_reexports)]

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("923UZSrzZJ2oiP9M9b4ABY2fGWpCsdZ7rnazZd3MJkzo");

#[program]
pub mod anchor_core_staking {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        rewards_bps: u16,
        freeze_period: u16,
        burn_rewards: u64,
    ) -> Result<()> {
        initialize::handler(ctx, rewards_bps, freeze_period, burn_rewards)
    }

    pub fn create_collection(
        ctx: Context<CreateCollection>,
        name: String,
        uri: String,
    ) -> Result<()> {
        create_collection::handler(ctx, name, uri)
    }

    pub fn mint_asset(ctx: Context<MintAsset>, name: String, uri: String) -> Result<()> {
        mint_asset::handler(ctx, name, uri)
    }

    pub fn stake(ctx: Context<Stake>) -> Result<()> {
        stake::handler(ctx)
    }

    pub fn unstake(ctx: Context<Unstake>) -> Result<()> {
        unstake::handler(ctx)
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        claim_rewards::handler(ctx)
    }

    pub fn burn_staked_nft(ctx: Context<BurnStakedNft>) -> Result<()> {
        burn_staked_nft::handler(ctx)
    }

    pub fn init_oracle(ctx: Context<InitOracle>, incentives: u64) -> Result<()> {
        init_oracle::handler(ctx, incentives)
    }

    pub fn update_oracle(ctx: Context<UpdateOracle>) -> Result<()> {
        update_oracle::handler(ctx)
    }

    pub fn transfer(ctx: Context<Transfer>) -> Result<()> {
        transfer::handler(ctx)
    }
}
