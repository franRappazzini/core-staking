import * as anchor from "@coral-xyz/anchor";

import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  burn,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { MPL_CORE_PROGRAM_ID, fetchCollection, mplCore } from "@metaplex-foundation/mpl-core";
import { lamports, publicKey } from "@metaplex-foundation/umi";

import { AnchorCoreStaking } from "../target/types/anchor_core_staking";
import { Program } from "@coral-xyz/anchor";
import { SystemProgram } from "@solana/web3.js";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { expect } from "chai";

const MILLISECONDS_PER_DAY = 86400000;
const REWARDS_BPS = 10000;
const BURN_REWARDS_AMOUNT = new anchor.BN(30 * 1e6); // one month of rewards per burn the nft (expressed in decimals)
const FREEZE_PERIOD_IN_DAYS = 7;
let TIME_TRAVEL_IN_DAYS = 8; // `let` because we update it in each travel

describe("anchor-core-staking", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.anchorCoreStaking as Program<AnchorCoreStaking>;

  const umi = createUmi(provider.connection).use(mplCore());

  const newOwner = anchor.web3.Keypair.generate(); // to receive transfers

  // Generate a keypair for the collection
  const collectionKeypair = anchor.web3.Keypair.generate();

  // Find the update authority for the collection (PDA)
  const updateAuthority = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("update_authority"), collectionKeypair.publicKey.toBuffer()],
    program.programId,
  )[0];

  // Generate a keypair for the nft asset
  const nftKeypair = anchor.web3.Keypair.generate();
  const secondNftKeypair = anchor.web3.Keypair.generate();
  const thirdNftKeypair = anchor.web3.Keypair.generate();

  // Find the config account (PDA)
  const config = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("config"), collectionKeypair.publicKey.toBuffer()],
    program.programId,
  )[0];

  // Find the rewards mint account (PDA)
  const rewardsMint = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("rewards_mint"), config.toBuffer()],
    program.programId,
  )[0];

  // Helper function to advance time with Surfpool
  async function advanceTime(params: {
    absoluteEpoch?: number;
    absoluteSlot?: number;
    absoluteTimestamp?: number;
  }): Promise<void> {
    const rpcResponse = await fetch(provider.connection.rpcEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "surfnet_timeTravel",
        params: [params],
      }),
    });

    const result = (await rpcResponse.json()) as { error?: any; result?: any };
    if (result.error) {
      throw new Error(`Time travel failed: ${JSON.stringify(result.error)}`);
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  it("Initialize Oracle account", async () => {
    const INCENTIVES = new anchor.BN(5_000 * 5); // base fee tx * 5

    const tx = await program.methods
      .initOracle(INCENTIVES)
      .accountsPartial({
        signer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);
  });

  it("Create a collection", async () => {
    const collectionName = "Test Collection";
    const collectionUri = "https://example.com/collection";
    const tx = await program.methods
      .createCollection(collectionName, collectionUri)
      .accountsPartial({
        payer: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([collectionKeypair])
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("Collection address", collectionKeypair.publicKey.toBase58());
  });

  it("Mint an NFT", async () => {
    const nftName = "Test NFT";
    const nftUri = "https://example.com/nft";
    const tx = await program.methods
      .mintAsset(nftName, nftUri)
      .accountsPartial({
        user: provider.wallet.publicKey,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([nftKeypair])
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("NFT address", nftKeypair.publicKey.toBase58());
  });

  it("Initialize Config", async () => {
    const tx = await program.methods
      .initialize(REWARDS_BPS, FREEZE_PERIOD_IN_DAYS, BURN_REWARDS_AMOUNT)
      .accountsPartial({
        admin: provider.wallet.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("Config address", config.toBase58());
    console.log("Rewards BPS", REWARDS_BPS);
    console.log("Freeze period in days", FREEZE_PERIOD_IN_DAYS);
    console.log("Rewards mint address", rewardsMint.toBase58());
  });

  it("Stake an NFT", async () => {
    const tx = await program.methods
      .stake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);

    // get collection attributes
    const collection = await fetchCollection(umi, publicKey(collectionKeypair.publicKey));
    const totalStaked = collection.attributes?.attributeList.find(
      (a) => a.key == "total_staked",
    )?.value;
    expect(Number(totalStaked)).equal(1);
  });

  it("Try to unstake an NFT before the freeze period ends", async () => {
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );
    try {
      const tx = await program.methods
        .unstake()
        .accountsPartial({
          owner: provider.wallet.publicKey,
          updateAuthority,
          config,
          rewardsMint,
          userRewardsAta,
          asset: nftKeypair.publicKey,
          collection: collectionKeypair.publicKey,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        })
        .rpc();
      throw new Error(
        `Unstake should have failed before freeze period elapsed, but succeeded with tx: ${tx}`,
      );
    } catch (err) {
      if (
        err instanceof anchor.AnchorError &&
        err.error.errorCode.code === "FreezePeriodNotElapsed"
      ) {
        console.log("\nUnstake failed as expected:", err.error.errorMessage);
      } else {
        throw err;
      }
    }
  });

  it("Time travel to the future", async () => {
    // Advance time in milliseconds
    const currentTimestamp = Date.now();
    await advanceTime({
      absoluteTimestamp: currentTimestamp + TIME_TRAVEL_IN_DAYS * MILLISECONDS_PER_DAY,
    });
    console.log("\nTime traveled in days", TIME_TRAVEL_IN_DAYS);

    TIME_TRAVEL_IN_DAYS *= 2; // we duplicate for the next use case
  });

  it("Claim Rewards (without unstake)", async () => {
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let beforeBalance = 0;
    try {
      beforeBalance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount as number;
    } catch (_) {
      // account not found, do nothing
    }

    const tx = await program.methods
      .claimRewards()
      .accountsStrict({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        userRewardsAta,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("\nYour transaction signature", tx);

    const balance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
      .uiAmount;

    expect(balance as number).greaterThan(beforeBalance);
  });

  it("Time travel to the future", async () => {
    // Advance time in milliseconds
    const currentTimestamp = Date.now();
    await advanceTime({
      absoluteTimestamp: currentTimestamp + TIME_TRAVEL_IN_DAYS * MILLISECONDS_PER_DAY,
    });
    console.log("\nTime traveled in days", TIME_TRAVEL_IN_DAYS);

    TIME_TRAVEL_IN_DAYS *= 2; // we duplicate for the next use case
  });

  it("Unstake an NFT", async () => {
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let beforeBalance = 0;
    try {
      beforeBalance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount as number;
    } catch (_) {
      // account not found, do nothing
    }

    const tx = await program.methods
      .unstake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        userRewardsAta,
        asset: nftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);

    const balance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
      .uiAmount;
    expect(balance).greaterThan(beforeBalance);

    // get collection attributes
    const collection = await fetchCollection(umi, publicKey(collectionKeypair.publicKey));
    const totalStaked = collection.attributes?.attributeList.find(
      (a) => a.key == "total_staked",
    )?.value;
    expect(Number(totalStaked)).equal(0);
  });

  // burn

  it("Mint a 2nd NFT", async () => {
    const nftName = "2nd NFT";
    const nftUri = "https://example.com/nft";
    const tx = await program.methods
      .mintAsset(nftName, nftUri)
      .accountsPartial({
        user: provider.wallet.publicKey,
        asset: secondNftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([secondNftKeypair])
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("NFT address", secondNftKeypair.publicKey.toBase58());
  });

  it("Stake the 2nd NFT", async () => {
    const tx = await program.methods
      .stake()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        asset: secondNftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);

    // get collection attributes
    const collection = await fetchCollection(umi, publicKey(collectionKeypair.publicKey));
    const totalStaked = collection.attributes?.attributeList.find(
      (a) => a.key == "total_staked",
    )?.value;
    expect(Number(totalStaked)).equal(1);
  });

  it("Time travel to the future", async () => {
    // Advance time in milliseconds
    const currentTimestamp = Date.now();
    await advanceTime({
      absoluteTimestamp: currentTimestamp + TIME_TRAVEL_IN_DAYS * MILLISECONDS_PER_DAY,
    });
    console.log("\nTime traveled in days", TIME_TRAVEL_IN_DAYS);

    TIME_TRAVEL_IN_DAYS *= 2; // we duplicate for the next use case
  });

  it("Burn staked NFT and claim one-time rewards", async () => {
    // Get the user rewards ATA account
    const userRewardsAta = getAssociatedTokenAddressSync(
      rewardsMint,
      provider.wallet.publicKey,
      false,
      TOKEN_PROGRAM_ID,
      ASSOCIATED_TOKEN_PROGRAM_ID,
    );

    let beforeBalance = 0;
    try {
      beforeBalance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
        .uiAmount as number;
    } catch (_) {
      // account not found, do nothing
    }

    const tx = await program.methods
      .burnStakedNft()
      .accountsStrict({
        owner: provider.wallet.publicKey,
        updateAuthority,
        config,
        rewardsMint,
        userRewardsAta,
        asset: secondNftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();

    console.log("\nYour transaction signature", tx);

    const balance = (await provider.connection.getTokenAccountBalance(userRewardsAta)).value
      .uiAmount;
    expect(balance).greaterThan(beforeBalance);

    const accoutData = await provider.connection.getAccountInfo(secondNftKeypair.publicKey);
    expect(accoutData?.data.length).equal(1);

    // get collection attributes
    const collection = await fetchCollection(umi, publicKey(collectionKeypair.publicKey));
    const totalStaked = collection.attributes?.attributeList.find(
      (a) => a.key == "total_staked",
    )?.value;
    expect(Number(totalStaked)).equal(0);
  });

  // transfer with oracle timestamp validation (verify current timestamp, it could fail)
  it("Mint a 3rd NFT", async () => {
    const nftName = "3rd NFT";
    const nftUri = "https://example.com/nft";
    const tx = await program.methods
      .mintAsset(nftName, nftUri)
      .accountsPartial({
        user: provider.wallet.publicKey,
        asset: thirdNftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        updateAuthority,
        systemProgram: SystemProgram.programId,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
      })
      .signers([thirdNftKeypair])
      .rpc();
    console.log("\nYour transaction signature", tx);
    console.log("NFT address", thirdNftKeypair.publicKey.toBase58());
  });

  it("Update Oracle to a valid timestamp", async () => {
    // first advance to a valid timestamp
    const harcodedTimestamp = 1853928000000; // Saturday, 30 September 2028 at 12:00:00 UTC

    // Advance time in milliseconds
    await advanceTime({
      absoluteTimestamp: harcodedTimestamp,
    });
    console.log("\nTime traveled to:", harcodedTimestamp);

    const beforeLamports = await provider.connection.getBalance(provider.wallet.publicKey);

    // update oracle
    const tx = await program.methods
      .updateOracle()
      .accountsPartial({
        signer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);

    // validate rewards
    const lamports = await provider.connection.getBalance(provider.wallet.publicKey);
    expect(lamports).greaterThan(beforeLamports);
  });

  it("Transfer NFT", async () => {
    const tx = await program.methods
      .transfer()
      .accountsPartial({
        owner: provider.wallet.publicKey,
        newOwner: newOwner.publicKey,
        asset: thirdNftKeypair.publicKey,
        collection: collectionKeypair.publicKey,
        mplCoreProgram: MPL_CORE_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);
  });

  it("Update Oracle to an invalid timestamp", async () => {
    // first advance to a valid timestamp
    const harcodedTimestamp = 1853949600000; // Saturday, 30 September 2028 at 18:00:00 UTC

    // Advance time in milliseconds
    await advanceTime({
      absoluteTimestamp: harcodedTimestamp,
    });
    console.log("\nTime traveled to:", harcodedTimestamp);

    const beforeLamports = await provider.connection.getBalance(provider.wallet.publicKey);

    // update oracle
    const tx = await program.methods
      .updateOracle()
      .accountsPartial({
        signer: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("\nYour transaction signature", tx);

    // validate rewards
    const lamports = await provider.connection.getBalance(provider.wallet.publicKey);
    expect(lamports).greaterThan(beforeLamports);
  });

  it("Fail to transfer NFT in not allowed timestamp", async () => {
    try {
      const tx = await program.methods
        .transfer()
        .accountsPartial({
          owner: newOwner.publicKey,
          newOwner: provider.wallet.publicKey,
          asset: thirdNftKeypair.publicKey,
          collection: collectionKeypair.publicKey,
          mplCoreProgram: MPL_CORE_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
        })
        .signers([newOwner])
        .rpc();
      console.log("\nYour transaction signature", tx);

      expect.fail("Must fail");
    } catch (err: any) {
      expect(err?.error?.errorCode?.code).equal("InvalidTimestampToTransfer");
    }
  });
});
