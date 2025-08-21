import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Lottery } from "../target/types/fbbuy";
import { assert } from "chai";

describe("fbbuy", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Lottery as Program<Lottery>;
  const admin = provider.wallet as anchor.Wallet;

  let lotteryPoolPda: anchor.web3.PublicKey;
  let bettor1: anchor.web3.Keypair;
  let bettor2: anchor.web3.Keypair;
  let bettor1BetPda: anchor.web3.PublicKey;
  let bettor2BetPda: anchor.web3.PublicKey;

  const matchId = new anchor.BN(12345);
  const betAmount = new anchor.BN(100);

  before(async () => {
    // 创建测试账户
    bettor1 = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(bettor1.publicKey, 10000000000);
    bettor2 = anchor.web3.Keypair.generate();
    await provider.connection.requestAirdrop(bettor2.publicKey, 10000000000);

    // 计算 PDA
    const [poolPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("lottery_pool"), matchId.toBuffer("le", 8)],
      program.programId
    );
    lotteryPoolPda = poolPda;

    const [bettor1Bet] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), bettor1.publicKey.toBuffer(), lotteryPoolPda.toBuffer()],
      program.programId
    );
    bettor1BetPda = bettor1Bet;

    const [bettor2Bet] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("bet"), bettor2.publicKey.toBuffer(), lotteryPoolPda.toBuffer()],
      program.programId
    );
    bettor2BetPda = bettor2Bet;
  });

  it("1. Creates a new lottery pool", async () => {
    const endTimestamp = new anchor.BN(Date.now() / 1000 + 3600); // 1小时后结束
    await program.methods
      .createLotteryPool(matchId, endTimestamp)
      .accounts({
        lotteryPool: lotteryPoolPda,
        admin: admin.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const pool = await program.account.lotteryPool.fetch(lotteryPoolPda);
    assert.equal(pool.totalBetAmount.toString(), "0");
  });

  it("2. Bettor 1 places a bet on HomeWin", async () => {
    await program.methods
      .placeBet(betAmount, { homeWin: {} })
      .accounts({
        betAccount: bettor1BetPda,
        lotteryPool: lotteryPoolPda,
        bettor: bettor1.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([bettor1])
      .rpc();

    const bet = await program.account.betAccount.fetch(bettor1BetPda);
    assert.equal(bet.betAmount.toString(), betAmount.toString());
    const pool = await program.account.lotteryPool.fetch(lotteryPoolPda);
    assert.equal(pool.totalBetAmount.toString(), betAmount.toString());
  });

  it("3. Bettor 2 places a bet on HomeWin", async () => {
    await program.methods
      .placeBet(betAmount, { homeWin: {} })
      .accounts({
        betAccount: bettor2BetPda,
        lotteryPool: lotteryPoolPda,
        bettor: bettor2.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([bettor2])
      .rpc();

    const bet = await program.account.betAccount.fetch(bettor2BetPda);
    assert.equal(bet.betAmount.toString(), betAmount.toString());
    const pool = await program.account.lotteryPool.fetch(lotteryPoolPda);
    assert.equal(pool.totalBetAmount.toString(), (betAmount.mul(new anchor.BN(2))).toString());
  });
});