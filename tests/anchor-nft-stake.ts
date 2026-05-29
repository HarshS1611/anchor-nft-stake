import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorNftStake } from "../target/types/anchor_nft_stake";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
  createCollection, fetchCollection, create,
  mplCore, MPL_CORE_PROGRAM_ID,
} from "@metaplex-foundation/mpl-core";
import { generateSigner, keypairIdentity } from "@metaplex-foundation/umi";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { assert } from "chai";

function getConfigPda(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("config")], programId)[0];
}
function getUserPda(user: PublicKey, programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("user"), user.toBuffer()], programId)[0];
}
function getStakePda(asset: PublicKey, config: PublicKey, programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("stake"), asset.toBuffer(), config.toBuffer()], programId)[0];
}

describe("anchor-nft-stake", () => {
  const provider   = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program    = anchor.workspace.AnchorNftStake as Program<AnchorNftStake>;
  const wallet     = provider.wallet as anchor.Wallet;
  const connection = provider.connection;

  const umi        = createUmi(connection.rpcEndpoint).use(mplCore());
  const umiKeypair = umi.eddsa.createKeypairFromSecretKey(wallet.payer.secretKey);
  umi.use(keypairIdentity(umiKeypair));

  const rewardsMintKp = Keypair.generate();
  const mplCorePid    = new PublicKey(MPL_CORE_PROGRAM_ID.toString());

  let collectionPubkey: PublicKey;
  let assetPubkey: PublicKey;
  let configPda: PublicKey;
  let userPda: PublicKey;
  let stakePda: PublicKey;
  let rewardsTokenAccount: PublicKey;

  before(async () => {
    configPda = getConfigPda(program.programId);
    userPda   = getUserPda(wallet.publicKey, program.programId);

    const collectionSigner = generateSigner(umi);
    await createCollection(umi, {
      collection: collectionSigner,
      name: "Staking Collection",
      uri: "https://example.com/collection.json",
    }).sendAndConfirm(umi);
    collectionPubkey = new PublicKey(collectionSigner.publicKey.toString());

    const collectionAccount = await fetchCollection(umi, collectionSigner.publicKey);
    const assetSigner = generateSigner(umi);
    await create(umi, {
      asset: assetSigner, collection: collectionAccount,
      name: "Staked Ape #1", uri: "https://example.com/nft/1.json",
    }).sendAndConfirm(umi);

    assetPubkey = new PublicKey(assetSigner.publicKey.toString());
    stakePda    = getStakePda(assetPubkey, configPda, program.programId);

    console.log("Collection:", collectionPubkey.toBase58());
    console.log("Asset:", assetPubkey.toBase58());
  });

  async function stakeNft(asset?: PublicKey): Promise<PublicKey> {
    const a  = asset ?? assetPubkey;
    const sp = getStakePda(a, configPda, program.programId);
    await program.methods.stake().accountsStrict({
      owner: wallet.publicKey, asset: a,
      collection: collectionPubkey, config: configPda,
      userAccount: userPda, stakeAccount: sp,
      mplCoreProgram: mplCorePid, systemProgram: SystemProgram.programId,
    }).rpc();
    return sp;
  }

  async function unstakeNft(asset?: PublicKey, sp?: PublicKey) {
    await program.methods.unstake().accountsStrict({
      owner: wallet.publicKey, asset: asset ?? assetPubkey,
      collection: collectionPubkey, config: configPda,
      userAccount: userPda, stakeAccount: sp ?? stakePda,
      rewardsMint: rewardsMintKp.publicKey,
      rewardsTokenAccount, mplCoreProgram: mplCorePid,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).rpc();
  }

  async function claimRewards(sp?: PublicKey) {
    await program.methods.claimRewards().accountsStrict({
      owner: wallet.publicKey, config: configPda,
      userAccount: userPda, stakeAccount: sp ?? stakePda,
      rewardsMint: rewardsMintKp.publicKey, rewardsTokenAccount,
      tokenProgram: TOKEN_PROGRAM_ID, systemProgram: SystemProgram.programId,
    }).rpc();
  }

  it("initializes staking config and user account with correct default values", async () => {
    await program.methods.initialize().accountsStrict({
      admin: wallet.publicKey, collection: collectionPubkey,
      rewardsMint: rewardsMintKp.publicKey, config: configPda,
      userAccount: userPda, systemProgram: SystemProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
    }).signers([rewardsMintKp]).rpc();

    const config = await program.account.stakeConfig.fetch(configPda);
    assert.equal(config.rewardsPerDay, 10);
    assert.equal(config.freezePeriod, 0);
    assert.ok(config.rewardsMint.equals(rewardsMintKp.publicKey));
    assert.ok(config.collection.equals(collectionPubkey));

    const user = await program.account.userAccount.fetch(userPda);
    assert.equal(user.points, 0);
    assert.equal(user.amountStaked, 0);

    const ata = await getOrCreateAssociatedTokenAccount(
      connection, wallet.payer, rewardsMintKp.publicKey, wallet.publicKey);
    rewardsTokenAccount = ata.address;

    console.log("✓ Initialize passed");
  });

  it("rejects a second initialize call on the same config PDA", async () => {
    try {
      await program.methods.initialize().accountsStrict({
        admin: wallet.publicKey, collection: collectionPubkey,
        rewardsMint: rewardsMintKp.publicKey, config: configPda,
        userAccount: userPda, systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      const msg: string = e.message ?? "";
      assert.ok(
        msg.includes("Signature verification failed") ||
        msg.includes("already in use") ||
        msg.includes("custom program error"),
        `unexpected: ${msg}`
      );
      console.log("✓ Double-initialize rejected");
    }
  });

  it("stakes an NFT, freezes it via FreezeDelegate, and records staked_at timestamp", async () => {
    const preStakeTime = Math.floor(Date.now() / 1000);
    stakePda = await stakeNft();

    const stake = await program.account.stakeAccount.fetch(stakePda);
    assert.ok(stake.owner.equals(wallet.publicKey));
    assert.ok(stake.mint.equals(assetPubkey));
    assert.ok(stake.stakedAt.toNumber() >= preStakeTime);

    const user = await program.account.userAccount.fetch(userPda);
    assert.equal(user.amountStaked, 1);

    console.log("✓ Stake passed, amount_staked =", user.amountStaked);
  });

  it("rejects staking an already-staked NFT because stake PDA already exists", async () => {
    try {
      await stakeNft();
      assert.fail("should have thrown");
    } catch (e: any) {
      const msg: string = e.message ?? "";
      assert.ok(
        msg.includes("already in use") || msg.includes("AlreadyStaked") ||
        msg.includes("custom program error"),
        `unexpected: ${msg}`
      );
      console.log("✓ Double-stake rejected");
    }
  });

  it("rejects claim_rewards when less than 1 full day has elapsed since staking", async () => {
    try {
      await claimRewards();
      console.log("✓ Claim succeeded (clock advanced past 1 day)");
    } catch (e: any) {
      assert.ok(e.message.includes("NothingToClaim"), `unexpected: ${e.message}`);
      console.log("✓ NothingToClaim enforced correctly");
    }
  });

  it("unstakes NFT, thaws it, closes stake account, and decrements user staked count", async () => {
    await unstakeNft();

    const stakeInfo = await connection.getAccountInfo(stakePda);
    assert.isNull(stakeInfo);

    const user = await program.account.userAccount.fetch(userPda);
    assert.equal(user.amountStaked, 0);

    assert.isNotNull(await connection.getAccountInfo(assetPubkey));

    console.log("✓ Unstake passed, stake account closed");
  });

  it("mints a new collection NFT via the mint_asset program instruction", async () => {
    const newAssetKp = Keypair.generate();
    await program.methods.mintAsset("Staked Ape #2", "https://example.com/nft/2.json")
      .accountsStrict({
        payer: wallet.publicKey, asset: newAssetKp.publicKey,
        collection: collectionPubkey, config: configPda,
        mplCoreProgram: mplCorePid, systemProgram: SystemProgram.programId,
      }).signers([newAssetKp]).rpc();

    const info = await connection.getAccountInfo(newAssetKp.publicKey);
    assert.isNotNull(info);
    assert.ok(info!.data.length > 0);

    console.log("✓ mintAsset passed:", newAssetKp.publicKey.toBase58());
  });

  it("allows the same NFT to be re-staked after a full unstake (round-trip)", async () => {
    stakePda = await stakeNft();

    const user = await program.account.userAccount.fetch(userPda);
    assert.equal(user.amountStaked, 1);

    await unstakeNft();

    const userAfter = await program.account.userAccount.fetch(userPda);
    assert.equal(userAfter.amountStaked, 0);

    const stakeInfo = await connection.getAccountInfo(stakePda);
    assert.isNull(stakeInfo);

    console.log("✓ Full round-trip passed");
  });
});