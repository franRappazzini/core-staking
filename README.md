# Core Staking Anchor Program

## Program summary

- init_oracle
- update_oracle
- create_collection
- mint_asset
- initialize
- stake
- unstake
- transfer
- burn_staked_nft
- claim_rewards

## Run the project with Anchor

### 1) Install prerequisites

Make sure you have the following installed:

- Rust
- Solana CLI
- Anchor CLI
- Surfpool CLI

### 2) Run the local test

```bash
# Sync anchor keys
anchor keys sync

# Build the program
anchor build

# Run Sufpool validator
surfpool start

# Run the tests using the surfpool validator
anchor test --skip-local-validator --skip-build --skip-deploy
```
