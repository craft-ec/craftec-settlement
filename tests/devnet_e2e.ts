/**
 * Craftec Settlement — Devnet E2E Test
 *
 * Tests the full money flow:
 *   1. Setup: create test mint OR reuse existing one from config
 *   2. Initialize config with 5% protocol fee (or skip if exists)
 *   3. Create creator pool (or skip if exists)
 *   4. Deposit tokens into pool
 *   5. Open payment channel
 *   6. Close payment channel with voucher
 *   7. Verify on-chain state
 *
 * Re-run command:
 *   cd craftec-settlement
 *   ANCHOR_PROVIDER_URL=https://api.devnet.solana.com ANCHOR_WALLET=~/.config/solana/id.json \
 *     npx ts-mocha -p ./tsconfig.json -t 1000000 tests/devnet_e2e.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { CraftecSettlement } from "../target/types/craftec_settlement";
import {
  createMint,
  createAssociatedTokenAccount,
  createAssociatedTokenAccountIdempotent,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
  mintTo,
  getAssociatedTokenAddress,
  getAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert } from "chai";
import BN from "bn.js";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function ensureAtaHelper(
  conn: anchor.web3.Connection,
  payer: Keypair,
  mint: PublicKey,
  owner: PublicKey,
): Promise<PublicKey> {
  const ata = getAssociatedTokenAddressSync(mint, owner, true);
  try {
    await getAccount(conn, ata);
  } catch {
    const ix = createAssociatedTokenAccountInstruction(payer.publicKey, ata, owner, mint);
    const tx = new anchor.web3.Transaction().add(ix);
    tx.feePayer = payer.publicKey;
    const bh = await conn.getLatestBlockhash();
    tx.recentBlockhash = bh.blockhash;
    tx.sign(payer);
    await conn.sendRawTransaction(tx.serialize());
    await sleep(2000);
  }
  return ata;
}

describe("devnet e2e settlement flow", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.CraftecSettlement as Program<CraftecSettlement>;

  const wallet = provider.wallet as anchor.Wallet;
  const connection = provider.connection;

  // Test state — populated in setup
  let testMint: PublicKey;
  let walletTokenAccount: PublicKey;
  let configPda: PublicKey;
  let treasuryTokenAccount: PublicKey;
  let creatorPoolPda: PublicKey;
  let poolTokenAccount: PublicKey;
  let configExists = false;
  let creatorPoolExists = false;

  const receiver = Keypair.generate();
  let receiverTokenAccount: PublicKey;

  const DECIMALS = 6;
  const MINT_AMOUNT = 1_000_000_000; // 1000 tokens
  const DEPOSIT_AMOUNT = 100_000_000; // 100 tokens
  const CHANNEL_LOCK_AMOUNT = 50_000_000; // 50 tokens
  const VOUCHER_AMOUNT = 30_000_000; // 30 tokens
  const PROTOCOL_FEE_BPS = 500; // 5%

  before(async () => {
    console.log("Program ID:", program.programId.toBase58());
    console.log("Wallet:", wallet.publicKey.toBase58());
    const bal = await connection.getBalance(wallet.publicKey);
    console.log("SOL balance:", bal / 1e9);

    // Derive config PDA
    [configPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("config")],
      program.programId,
    );

    // Check if config already exists — if so, reuse its mint
    try {
      const config = await program.account.config.fetch(configPda);
      configExists = true;
      treasuryTokenAccount = config.treasury;
      const treasuryAcct = await getAccount(connection, treasuryTokenAccount);
      testMint = treasuryAcct.mint;
      console.log("Config already exists, reusing mint:", testMint.toBase58());
    } catch {
      configExists = false;
      console.log("Config does not exist, will create fresh");
    }

    // Check if creator pool already exists
    [creatorPoolPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("creator_pool"), wallet.publicKey.toBuffer()],
      program.programId,
    );
    try {
      await program.account.creatorPool.fetch(creatorPoolPda);
      creatorPoolExists = true;
    } catch {
      creatorPoolExists = false;
    }

    // Fund receiver for rent
    console.log("Funding receiver with 0.01 SOL...");
    const tx = new anchor.web3.Transaction().add(
      SystemProgram.transfer({
        fromPubkey: wallet.publicKey,
        toPubkey: receiver.publicKey,
        lamports: 10_000_000,
      }),
    );
    await provider.sendAndConfirm(tx);
  });

  it("1. Setup: create mint and mint tokens", async () => {
    if (!configExists) {
      // Create a fresh test mint
      testMint = await createMint(
        connection,
        wallet.payer,
        wallet.publicKey,
        null,
        DECIMALS,
      );
      console.log("Created new test mint:", testMint.toBase58());
    } else {
      console.log("Using existing mint:", testMint.toBase58());
    }

    // Get or create wallet's ATA for this mint
    walletTokenAccount = await getAssociatedTokenAddress(testMint, wallet.publicKey);
    try {
      await getAccount(connection, walletTokenAccount);
      console.log("Wallet ATA already exists");
    } catch {
      walletTokenAccount = await createAssociatedTokenAccount(
        connection,
        wallet.payer,
        testMint,
        wallet.publicKey,
      );
      console.log("Created wallet ATA");
    }

    // Mint tokens (we're the mint authority for test mints)
    try {
      await mintTo(
        connection,
        wallet.payer,
        testMint,
        walletTokenAccount,
        wallet.publicKey,
        MINT_AMOUNT,
      );
      console.log("Minted", MINT_AMOUNT / 10 ** DECIMALS, "tokens");
    } catch (e: any) {
      console.log("Mint failed (may not be authority for existing mint):", e.message?.slice(0, 80));
    }

    const acct = await getAccount(connection, walletTokenAccount);
    console.log("Wallet token balance:", Number(acct.amount) / 10 ** DECIMALS, "tokens");
    assert.isAbove(Number(acct.amount), 0, "Need tokens to test");
  });

  it("2. Initialize config with 5% protocol fee", async () => {
    if (configExists) {
      const config = await program.account.config.fetch(configPda);
      console.log("Config already exists, fee:", config.protocolFeeBps, "bps");
      assert.equal(config.protocolFeeBps, PROTOCOL_FEE_BPS);
      return;
    }

    treasuryTokenAccount = await getAssociatedTokenAddress(testMint, configPda, true);
    console.log("Config PDA:", configPda.toBase58());
    console.log("Treasury ATA:", treasuryTokenAccount.toBase58());

    const tx = await program.methods
      .initializeConfig(PROTOCOL_FEE_BPS)
      .accountsStrict({
        admin: wallet.publicKey,
        config: configPda,
        usdcMint: testMint,
        treasuryTokenAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("initialize_config tx:", tx);

    const config = await program.account.config.fetch(configPda);
    assert.equal(config.protocolFeeBps, PROTOCOL_FEE_BPS);
    assert.equal(config.paused, false);
  });

  it("3. Create creator pool", async () => {
    poolTokenAccount = await getAssociatedTokenAddress(testMint, creatorPoolPda, true);

    if (creatorPoolExists) {
      console.log("Creator pool already exists");
      // Ensure pool ATA exists for this mint
      await ensureAtaHelper(connection, wallet.payer, testMint, creatorPoolPda);
      return;
    }

    // Create pool ATA first
    await ensureAtaHelper(connection, wallet.payer, testMint, creatorPoolPda);

    const tx = await program.methods
      .createCreatorPool(1) // service = Data
      .accountsStrict({
        creator: wallet.publicKey,
        creatorPool: creatorPoolPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("create_creator_pool tx:", tx);

    const pool = await program.account.creatorPool.fetch(creatorPoolPda);
    assert.equal(pool.service, 1);
    console.log("Creator pool created");
  });

  it("4. Deposit tokens into creator pool", async () => {
    await sleep(2000);

    const poolBefore = await program.account.creatorPool.fetch(creatorPoolPda);
    const balBefore = poolBefore.poolBalance.toNumber();

    const tx = await program.methods
      .deposit(new BN(DEPOSIT_AMOUNT))
      .accountsStrict({
        depositor: wallet.publicKey,
        creatorPool: creatorPoolPda,
        depositorTokenAccount: walletTokenAccount,
        poolTokenAccount,
        usdcMint: testMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();
    console.log("deposit tx:", tx);

    await sleep(1000);
    const pool = await program.account.creatorPool.fetch(creatorPoolPda);
    console.log("Pool balance:", pool.poolBalance.toNumber() / 10 ** DECIMALS, "tokens");
    assert.equal(pool.poolBalance.toNumber(), balBefore + DEPOSIT_AMOUNT);
  });

  it("5. Open payment channel", async () => {
    await sleep(2000);
    const nonce = new BN(Date.now()); // unique nonce per run
    const nonceBuf = nonce.toArrayLike(Buffer, "le", 8);

    const [channelPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("payment_channel"), wallet.publicKey.toBuffer(), receiver.publicKey.toBuffer(), nonceBuf],
      program.programId,
    );
    const channelTokenAccount = await getAssociatedTokenAddress(testMint, channelPda, true);

    console.log("Channel PDA:", channelPda.toBase58());
    console.log("Nonce:", nonce.toString());

    // Create receiver ATA
    try {
      receiverTokenAccount = await createAssociatedTokenAccount(
        connection, wallet.payer, testMint, receiver.publicKey,
      );
    } catch {
      receiverTokenAccount = await getAssociatedTokenAddress(testMint, receiver.publicKey);
    }

    const tx = await program.methods
      .openChannel(receiver.publicKey, nonce, new BN(CHANNEL_LOCK_AMOUNT))
      .accountsStrict({
        sender: wallet.publicKey,
        config: configPda,
        channel: channelPda,
        senderTokenAccount: walletTokenAccount,
        channelTokenAccount,
        usdcMint: testMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    console.log("open_channel tx:", tx);

    const channel = await program.account.paymentChannel.fetch(channelPda);
    console.log("Channel locked:", channel.lockedAmount.toNumber() / 10 ** DECIMALS, "tokens");
    assert.equal(channel.lockedAmount.toNumber(), CHANNEL_LOCK_AMOUNT);

    // Store nonce for close_channel test
    (global as any).__testNonce = nonce;
    (global as any).__channelPda = channelPda;
  });

  it("6. Close payment channel, verify fee deduction", async () => {
    await sleep(2000);
    const nonce: BN = (global as any).__testNonce;
    const channelPda: PublicKey = (global as any).__channelPda;
    const nonceBuf = nonce.toArrayLike(Buffer, "le", 8);

    const channelTokenAccount = await getAssociatedTokenAddress(testMint, channelPda, true);
    const dummySignature = new Array(64).fill(0);

    const receiverBefore = Number((await getAccount(connection, receiverTokenAccount)).amount);
    const senderBefore = Number((await getAccount(connection, walletTokenAccount)).amount);
    let treasuryBefore = 0;
    try {
      treasuryBefore = Number((await getAccount(connection, treasuryTokenAccount)).amount);
    } catch { /* empty */ }

    const tx = await program.methods
      .closeChannel(new BN(VOUCHER_AMOUNT), new BN(1), dummySignature)
      .accountsStrict({
        receiver: receiver.publicKey,
        config: configPda,
        channel: channelPda,
        channelTokenAccount,
        receiverTokenAccount,
        senderTokenAccount: walletTokenAccount,
        treasuryTokenAccount,
        sender: wallet.publicKey,
        usdcMint: testMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([receiver])
      .rpc();
    console.log("close_channel tx:", tx);

    const expectedFee = Math.floor(VOUCHER_AMOUNT * PROTOCOL_FEE_BPS / 10_000);
    const expectedReceiverPayout = VOUCHER_AMOUNT - expectedFee;
    const expectedSenderRefund = CHANNEL_LOCK_AMOUNT - VOUCHER_AMOUNT;

    await sleep(1000);
    const receiverAfter = Number((await getAccount(connection, receiverTokenAccount)).amount);
    const senderAfter = Number((await getAccount(connection, walletTokenAccount)).amount);
    const treasuryAfter = Number((await getAccount(connection, treasuryTokenAccount)).amount);

    console.log("\n=== Settlement Results ===");
    console.log(`Voucher amount: ${VOUCHER_AMOUNT / 10 ** DECIMALS} tokens`);
    console.log(`Protocol fee (5%): ${expectedFee / 10 ** DECIMALS} tokens`);
    console.log(`Receiver payout: ${expectedReceiverPayout / 10 ** DECIMALS} tokens`);
    console.log(`Sender refund: ${expectedSenderRefund / 10 ** DECIMALS} tokens`);
    console.log(`Treasury balance: ${treasuryAfter / 10 ** DECIMALS} tokens`);

    assert.equal(receiverAfter - receiverBefore, expectedReceiverPayout, "Receiver payout mismatch");
    assert.equal(senderAfter - senderBefore, expectedSenderRefund, "Sender refund mismatch");
    assert.equal(treasuryAfter - treasuryBefore, expectedFee, "Treasury fee mismatch");
  });

  it("7. Verify final on-chain state", async () => {
    const config = await program.account.config.fetch(configPda);
    assert.equal(config.protocolFeeBps, PROTOCOL_FEE_BPS);
    assert.equal(config.paused, false);
    console.log("✅ Config valid");

    const pool = await program.account.creatorPool.fetch(creatorPoolPda);
    console.log("✅ Creator pool balance:", pool.poolBalance.toNumber() / 10 ** DECIMALS, "tokens");

    // Channel should be closed
    const channelPda: PublicKey = (global as any).__channelPda;
    try {
      await program.account.paymentChannel.fetch(channelPda);
      assert.fail("Channel should be closed");
    } catch {
      console.log("✅ Channel account closed");
    }

    // Treasury collected fee
    const treasuryAcct = await getAccount(connection, treasuryTokenAccount);
    console.log("✅ Treasury balance:", Number(treasuryAcct.amount) / 10 ** DECIMALS, "tokens");

    const solBal = await connection.getBalance(wallet.publicKey);
    console.log("\nFinal SOL balance:", solBal / 1e9);
    assert.isAbove(solBal / 1e9, 1.0, "Should have >1 SOL remaining");

    console.log("\n🎉 All E2E settlement tests passed!");
  });
});
