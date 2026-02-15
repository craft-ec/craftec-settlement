import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CraftecSettlement } from "../target/types/craftec_settlement";
import {
  createMint,
  createAssociatedTokenAccount,
  mintTo,
  getAssociatedTokenAddress,
  getAccount,
} from "@solana/spl-token";
import { assert } from "chai";

describe("craftec-settlement", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .CraftecSettlement as Program<CraftecSettlement>;
  const admin = provider.wallet;

  let usdcMint: anchor.web3.PublicKey;
  let adminUsdcAta: anchor.web3.PublicKey;
  let configPda: anchor.web3.PublicKey;
  let configBump: number;
  let treasuryAta: anchor.web3.PublicKey;

  before(async () => {
    // Create USDC-like mint
    usdcMint = await createMint(
      provider.connection,
      (admin as any).payer,
      admin.publicKey,
      null,
      6
    );

    // Derive config PDA
    [configPda, configBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId
    );

    // Derive treasury ATA (owned by config PDA)
    treasuryAta = await getAssociatedTokenAddress(usdcMint, configPda, true);

    // Create admin's USDC ATA and mint some tokens
    adminUsdcAta = await createAssociatedTokenAccount(
      provider.connection,
      (admin as any).payer,
      usdcMint,
      admin.publicKey
    );
    await mintTo(
      provider.connection,
      (admin as any).payer,
      usdcMint,
      adminUsdcAta,
      admin.publicKey,
      1_000_000_000 // 1000 USDC
    );
  });

  it("initializes config", async () => {
    await program.methods
      .initializeConfig(500) // 5% fee
      .accounts({
        admin: admin.publicKey,
        config: configPda,
        usdcMint,
        treasuryTokenAccount: treasuryAta,
      })
      .rpc();

    const config = await program.account.config.fetch(configPda);
    assert.equal(config.protocolFeeBps, 500);
    assert.ok(config.admin.equals(admin.publicKey));
    assert.equal(config.paused, false);
  });

  it("rejects fee > 50%", async () => {
    try {
      await program.methods
        .updateConfig(5001, null, null)
        .accounts({ admin: admin.publicKey, config: configPda })
        .rpc();
      assert.fail("Should have thrown");
    } catch (e: any) {
      assert.include(e.toString(), "FeeTooHigh");
    }
  });

  it("updates config", async () => {
    await program.methods
      .updateConfig(300, null, null) // 3% fee
      .accounts({ admin: admin.publicKey, config: configPda })
      .rpc();

    const config = await program.account.config.fetch(configPda);
    assert.equal(config.protocolFeeBps, 300);
  });

  describe("creator pool", () => {
    let creatorPoolPda: anchor.web3.PublicKey;
    let poolAta: anchor.web3.PublicKey;

    it("creates a creator pool", async () => {
      [creatorPoolPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("creator_pool"), admin.publicKey.toBuffer()],
        program.programId
      );

      await program.methods
        .createCreatorPool(1) // Data service
        .accounts({
          creator: admin.publicKey,
          creatorPool: creatorPoolPda,
        })
        .rpc();

      const pool = await program.account.creatorPool.fetch(creatorPoolPda);
      assert.ok(pool.creator.equals(admin.publicKey));
      assert.equal(pool.service, 1);
      assert.equal(pool.poolBalance.toNumber(), 0);
    });

    it("deposits USDC into creator pool", async () => {
      poolAta = await getAssociatedTokenAddress(
        usdcMint,
        creatorPoolPda,
        true
      );

      // Create pool ATA first
      await createAssociatedTokenAccount(
        provider.connection,
        (admin as any).payer,
        usdcMint,
        creatorPoolPda,
        true // allowOwnerOffCurve
      );

      await program.methods
        .deposit(new anchor.BN(100_000_000)) // 100 USDC
        .accounts({
          depositor: admin.publicKey,
          creatorPool: creatorPoolPda,
          depositorTokenAccount: adminUsdcAta,
          poolTokenAccount: poolAta,
          usdcMint,
        })
        .rpc();

      const pool = await program.account.creatorPool.fetch(creatorPoolPda);
      assert.equal(pool.poolBalance.toNumber(), 100_000_000);

      const poolToken = await getAccount(provider.connection, poolAta);
      assert.equal(Number(poolToken.amount), 100_000_000);
    });

    it("rejects zero deposit", async () => {
      try {
        await program.methods
          .deposit(new anchor.BN(0))
          .accounts({
            depositor: admin.publicKey,
            creatorPool: creatorPoolPda,
            depositorTokenAccount: adminUsdcAta,
            poolTokenAccount: poolAta,
            usdcMint,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "InvalidAmount");
      }
    });
  });

  describe("payment channel", () => {
    const receiver = anchor.web3.Keypair.generate();
    let channelPda: anchor.web3.PublicKey;
    let channelAta: anchor.web3.PublicKey;
    const nonce = new anchor.BN(0);

    before(async () => {
      // Airdrop to receiver for rent
      const sig = await provider.connection.requestAirdrop(
        receiver.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig);
    });

    it("opens a payment channel", async () => {
      const nonceBuf = Buffer.alloc(8);
      nonceBuf.writeBigUInt64LE(0n);

      [channelPda] = anchor.web3.PublicKey.findProgramAddressSync(
        [
          Buffer.from("payment_channel"),
          admin.publicKey.toBuffer(),
          receiver.publicKey.toBuffer(),
          nonceBuf,
        ],
        program.programId
      );

      channelAta = await getAssociatedTokenAddress(
        usdcMint,
        channelPda,
        true
      );

      await program.methods
        .openChannel(receiver.publicKey, nonce, new anchor.BN(50_000_000)) // 50 USDC
        .accounts({
          sender: admin.publicKey,
          config: configPda,
          channel: channelPda,
          senderTokenAccount: adminUsdcAta,
          channelTokenAccount: channelAta,
          usdcMint,
        })
        .rpc();

      const channel = await program.account.paymentChannel.fetch(channelPda);
      assert.ok(channel.sender.equals(admin.publicKey));
      assert.ok(channel.receiver.equals(receiver.publicKey));
      assert.equal(channel.lockedAmount.toNumber(), 50_000_000);
      assert.equal(channel.voucherAmount.toNumber(), 0);
      assert.equal(channel.forceCloseSlot.toNumber(), 0);
    });

    it("initiates force close", async () => {
      const receiverUsdcAta = await createAssociatedTokenAccount(
        provider.connection,
        (admin as any).payer,
        usdcMint,
        receiver.publicKey
      );

      await program.methods
        .forceClose()
        .accounts({
          sender: admin.publicKey,
          config: configPda,
          channel: channelPda,
          channelTokenAccount: channelAta,
          receiverTokenAccount: receiverUsdcAta,
          senderTokenAccount: adminUsdcAta,
          treasuryTokenAccount: treasuryAta,
          usdcMint,
        })
        .rpc();

      const channel = await program.account.paymentChannel.fetch(channelPda);
      assert.ok(channel.forceCloseSlot.toNumber() > 0);
    });

    it("rejects force close finalize before timeout", async () => {
      const receiverUsdcAta = await getAssociatedTokenAddress(
        usdcMint,
        receiver.publicKey
      );

      try {
        await program.methods
          .forceClose()
          .accounts({
            sender: admin.publicKey,
            config: configPda,
            channel: channelPda,
            channelTokenAccount: channelAta,
            receiverTokenAccount: receiverUsdcAta,
            senderTokenAccount: adminUsdcAta,
            treasuryTokenAccount: treasuryAta,
            usdcMint,
          })
          .rpc();
        assert.fail("Should have thrown");
      } catch (e: any) {
        assert.include(e.toString(), "ForceCloseTimeout");
      }
    });
  });
});
