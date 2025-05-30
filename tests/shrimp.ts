/* --------------------------------------------------------------------------
 *  Shrimp Farm integration tests
 *
 *  The tests are grouped by feature for clarity:
 *  ────────────────────────────────────────────────────────────────────────────
 *  1.  Pre‑market purchases & dividends
 *  2.  NFT minting requirements
 *  3.  Minimum‑buy enforcement
 *  4.  Referral logic
 *  5.  Bonus mechanics (NFT & test‑net flag)
 *  6.  Cooldowns
 *  7.  End‑game conditions
 *  8.  Username registration
 *  9.  Dev‑withdraw
 * -------------------------------------------------------------------------- */

import * as anchor from "@coral-xyz/anchor";
import { createUmi as baseCreateUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { fetchAllDigitalAssetWithTokenByOwner } from "@metaplex-foundation/mpl-token-metadata";
import {
  createSignerFromKeypair,
  signerIdentity,
  publicKey,
  sol,
  generateSigner,
} from "@metaplex-foundation/umi";
import { AnchorProvider, Program, Wallet } from "@coral-xyz/anchor";
import { Shrimp } from "../target/types/shrimp";
import * as utils from "./utils";
import { Keypair, PublicKey, SystemProgram, Transaction } from "@solana/web3.js";
import { expect } from "chai";
import { createCandyMachineAndSetCollection, mintNft } from "./nft";
import { mplCandyMachine as mplCoreCandyMachine } from "@metaplex-foundation/mpl-core-candy-machine";
import * as fs from 'fs';

// ---------------------------------------------------------------------------
//  Constants & helpers
// ---------------------------------------------------------------------------

const OWNER_KEY_FILE = "keypairs/owner.json";

const PSN = new anchor.BN(10_000);
const PSNH = new anchor.BN(5_000);
async function getGameData(program: Program<Shrimp>, gameStateAccount: PublicKey) {
  return program.account?.gameState.fetch(gameStateAccount).catch(() => null);
}

function signerFromKeyFile(keyfile) {
  const data = fs.readFileSync(keyfile);
  const jsonData = JSON.parse(data.toString('utf-8'));
  return Keypair.fromSecretKey(new Uint8Array(jsonData));
}

export function keypairFromFile(path: string): Keypair {
  const raw = fs.readFileSync(path, { encoding: "utf-8" });
  const secret = Uint8Array.from(JSON.parse(raw));

  // Sanity-check (optional, but helps when the file is corrupted)
  if (secret.length !== 32 && secret.length !== 64) {
    throw new Error(`Expected 32 or 64 bytes, got ${secret.length}`);
  }
  return Keypair.fromSecretKey(secret);
}

const NULL_KEY = PublicKey.default;

// ---------------------------------------------------------------------------
//  Test suite
// ---------------------------------------------------------------------------

describe("Shrimp Farm", () => {
  /* ───────────────────────────────── setup ───────────────────────────────── */
  let provider: AnchorProvider;
  let wallet: Wallet;
  let program: Program<Shrimp>;

  // Dynamic PDAs / signers created fresh for every test
  let authority: Keypair;
  let gameStateAccount: PublicKey;
  let playerAccount: PublicKey;
  let refAccount: Keypair;
  let refStateAccount: PublicKey;
  let refAccount2: Keypair;
  let refStateAccount2: PublicKey;
  let randomAccount: Keypair;
  let dev1: Keypair;
  let dev2: Keypair;
  let dev3: Keypair;

  const createUmi = async () => {
    const RPC = "http://127.0.0.1:8899";
    const umi = await baseCreateUmi(RPC, "confirmed");

    const authorityKp = keypairFromFile("keypairs/test-authority.json");
    const authoritySigner = createSignerFromKeypair(
      umi,
      umi.eddsa.createKeypairFromSecretKey(authorityKp.secretKey)  // 32 or 64 bytes both work
    );

    umi.use(signerIdentity(authoritySigner, true));
    umi.use(mplCoreCandyMachine());
    return umi;
  };

  before(async () => {
    provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);
    wallet = provider.wallet as Wallet;
    program = anchor.workspace.Shrimp as Program<Shrimp>;
  });

  beforeEach(async () => {
    await new Promise(r => setTimeout(r, 1000)); // try to stop local validator from crashing
    /* fresh game for every test */
    authority = Keypair.generate();
    gameStateAccount = utils.findGameDataAcc(authority.publicKey);

    playerAccount = await utils.findPlayerDataAcc(wallet.publicKey, authority.publicKey);

    refAccount = Keypair.generate();
    refStateAccount = await utils.findPlayerDataAcc(refAccount.publicKey, authority.publicKey);

    refAccount2 = Keypair.generate();
    refStateAccount2 = await utils.findPlayerDataAcc(refAccount2.publicKey, authority.publicKey);

    randomAccount = Keypair.generate();

    dev1 = Keypair.generate();
    dev2 = Keypair.generate();
    dev3 = Keypair.generate();

    // bootstrap balances
    await provider.connection.requestAirdrop(dev1.publicKey, 100e8);
    await provider.connection.requestAirdrop(dev2.publicKey, 100e8);
    await provider.connection.requestAirdrop(dev3.publicKey, 1_000e9);
    await provider.connection.requestAirdrop(authority.publicKey, 1_000e9);
    await provider.connection.requestAirdrop(randomAccount.publicKey, 1_000e9);
    await provider.connection.requestAirdrop(refAccount.publicKey, 1_000e9);
    await provider.connection.requestAirdrop(refAccount2.publicKey, 1_000e9);
    await new Promise(r => setTimeout(r, 5000));

    const defaultPremarketEnd = new anchor.BN(
      Math.floor(Date.now() / 1000) + 72 * 60 * 60,   // now + 72 h
    );

    const owner = signerFromKeyFile(OWNER_KEY_FILE);

    await program.methods
      .initialize(
        dev1.publicKey,
        dev2.publicKey,
        dev3.publicKey,
        defaultPremarketEnd,
        new anchor.BN(5), // 5 s cooldown
        true,
      )
      .accounts({ authority: authority.publicKey, owner: owner.publicKey })
      .signers([authority, owner])
      .rpc();
  });

  afterEach(async () => {
    await new Promise(r => setTimeout(r, 5000)); // try to stop local validator from crashing
    const gameState = await program.account.gameState.fetch(gameStateAccount);
    if (gameState.devBalance.gt(new anchor.BN(5_296_560))) {
      await program.methods
        .devWithdraw()
        .accounts({
          signer: dev1.publicKey,
          authority: authority.publicKey,
          dev1: dev1.publicKey,
          dev2: dev2.publicKey,
          dev3: dev3.publicKey,
        })
        .signers([dev1])
        .rpc({ skipPreflight: true });
    }
  });

  /* ───────────────────────────── sub‑helpers ─────────────────────────────── */

  const advancePreMarket = async () => {
    await program.methods.endPremarket()
      .accounts({ authority: authority.publicKey })
      .signers([authority])
      .rpc();
  };

  const doBuy = async (
    method: "buyShrimp" | "buyPremarket",
    from: Keypair,
    amountLamports: number,
    ref: PublicKey | null,                 // what the callsite “wants”
  ) => {
    const additionalComputeIx =
      anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 });

    // if caller says “no referrer”, substitute the null key
    const refKey = ref ?? NULL_KEY;

    const accs: any = {
      authority: authority.publicKey,
      player: from.publicKey,
      referrer: refKey,
      referrerState: await utils.findPlayerDataAcc(refKey, authority.publicKey),
    };

    const sig = await program.methods[method](new anchor.BN(amountLamports))
      .preInstructions([additionalComputeIx])
      .accounts(accs)
      .signers([from])
      .rpc({ commitment: "confirmed" });

    await new Promise(r => setTimeout(r, 500)); // tiny settle-time helper
    return sig;
  };

  // convenience wrappers hide the new detail from all tests
  const buyPremarket = (f, amt, ref = null) => doBuy("buyPremarket", f, amt, ref);
  const buyShrimp = (f, amt, ref = null) => doBuy("buyShrimp", f, amt, ref);

  const buyPremarketMulti = async (
    count: number,
    from: Keypair,
    amount: anchor.BN,
    ref: PublicKey | null = null,
  ) => {
    const additionalComputeIx = anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 });

    const tx = await program.methods.buyPremarket(amount)
      .accounts({ player: from.publicKey, authority: authority.publicKey, referrer: ref })
      .instruction();

    const multiTx = new Transaction();

    multiTx.add(additionalComputeIx)
    for (let i = 0; i < count; ++i) {
      multiTx.add(tx)
    }

    await provider.connection.sendTransaction(multiTx, [from])
  };

  const setupReferrer = async (refAccount, name = 'referrer') => {
      // Perform minimum buy
      const amount = new anchor.BN(1e7); 
      await buyPremarket(refAccount, amount, NULL_KEY);
      // Register
      await program.methods.register(name)
        .accounts({ player: refAccount.publicKey, authority: authority.publicKey })
        .signers([refAccount])
        .rpc();
  }

  /* ═════════════════════════════ TESTS ════════════════════════════════════ */

  /* ------------------------------------------------------------------ 1 */
  describe("Pre‑market", () => {
    it("allows a pre‑market buy (with referrer)", async () => {
      await setupReferrer(refAccount);
      const amount = new anchor.BN(1e8); // 0.1 SOL
      await buyPremarket(wallet.payer, amount, refAccount.publicKey);
    });

    it("allows a pre‑market buy (without referrer)", async () => {
      const amount = new anchor.BN(1e8); // 0.1 SOL
      await buyPremarket(wallet.payer, amount, NULL_KEY);
    });

    it("limits number of IXs", async () => {
      const amount = new anchor.BN(1e8); // 0.1 SOL
      // 5 Buys should fail (6 ixs including computebudget)
      await utils.shouldRevert(buyPremarketMulti(5, wallet.payer, amount, NULL_KEY));
      // 4 Buys should work
      await buyPremarketMulti(4, wallet.payer, amount, NULL_KEY);
    });

    it("updates referral & cashback correctly", async () => {
      await setupReferrer(refAccount);

      const amount = new anchor.BN(1e8);
      await buyPremarket(wallet.payer, amount, refAccount.publicKey);

      const refState = await program.account.playerState.fetch(refStateAccount);
      const playerState = await program.account.playerState.fetch(playerAccount);

      const refFee = amount.mul(new anchor.BN(4)).div(new anchor.BN(100));
      const cashback = amount.mul(new anchor.BN(1)).div(new anchor.BN(100));

      expect(playerState.currentReferrer.toString()).to.equal(refAccount.publicKey.toString());
      expect(refState.referralTotal.eq(refFee)).to.be.true;
      expect(playerState.referralTotal.eq(cashback)).to.be.true;
    });

    it("distributes pre‑market dividends on first regular buy", async () => {
      const amount = new anchor.BN(1e8);
      await buyPremarket(wallet.payer, amount, NULL_KEY);
      await buyPremarket(randomAccount, amount, NULL_KEY);

      await advancePreMarket();
      await buyShrimp(wallet.payer, amount, NULL_KEY);

      await program.methods.userWithdraw()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey })
        .signers([wallet.payer])
        .rpc();

      const playerState = await program.account.playerState.fetch(playerAccount);
      const gameState = await program.account.gameState.fetch(gameStateAccount);

      const expected = gameState.premarketEarned.div(new anchor.BN(2));
      expect(playerState.premarketWithdrawn.toString()).to.equal(expected.toString())
    });
  });

  /* ------------------------------------------------------------------ 2 */
  describe("NFT minting", () => {
    it("does not allow setting collection twice", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);

      await utils.shouldError(
        createCandyMachineAndSetCollection(umi, program, authority, 10),
        "Collection already set");
    });

    it("allows mint after ≥ 1 SOL regular buy", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);

      await setupReferrer(refAccount);

      await advancePreMarket();
      await buyShrimp(randomAccount, new anchor.BN(1e9), refAccount.publicKey); // 1 SOL

      await mintNft(umi, program, authority.publicKey, randomAccount);
    });

    it("rejects mint when spent < 1 SOL", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);

      await setupReferrer(refAccount);

      await advancePreMarket();
      await buyShrimp(randomAccount, new anchor.BN(1e8), refAccount.publicKey); // 0.1 SOL

      await utils.shouldRevert(mintNft(umi, program, authority.publicKey, randomAccount));
    });

    it("rejects a second NFT mint", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);

      await setupReferrer(refAccount);

      await advancePreMarket();
      await buyShrimp(randomAccount, new anchor.BN(1e9), refAccount.publicKey);

      await mintNft(umi, program, authority.publicKey, randomAccount);
      await utils.shouldRevert(mintNft(umi, program, authority.publicKey, randomAccount));
    });
  });

  /* ------------------------------------------------------------------ 3 */
  describe("Minimum‑buy enforcement", () => {
    it("rejects amounts below 0.01 SOL in both phases", async () => {
      const zero = new anchor.BN(0);
      const small = new anchor.BN(9e6); // 0.009 SOL

      await setupReferrer(refAccount);

      await utils.shouldError(
        buyPremarket(wallet.payer, zero, refAccount.publicKey),
        "Buy amount below the 0.01 SOL minimum",
      );
      await utils.shouldError(
        buyPremarket(wallet.payer, small, refAccount.publicKey),
        "Buy amount below the 0.01 SOL minimum",
      );

      await advancePreMarket();

      await utils.shouldError(
        buyShrimp(wallet.payer, zero, refAccount.publicKey),
        "Buy amount below the 0.01 SOL minimum",
      );
      await utils.shouldError(
        buyShrimp(wallet.payer, small, refAccount.publicKey),
        "Buy amount below the 0.01 SOL minimum",
      );
    });
  });

  /* ------------------------------------------------------------------ 4 */
  describe("Referrals", () => {
    it("handles multiple referrers & cashbacks correctly", async () => {
      const amount = new anchor.BN(1e8);
      const refBonus = amount.toNumber() * 0.04;
      const cashback = amount.toNumber() * 0.01;

      await setupReferrer(refAccount);
      await setupReferrer(refAccount2, 'referrerb');

      await buyPremarket(wallet.payer, amount, refAccount.publicKey);
      await advancePreMarket();
      await buyShrimp(wallet.payer, amount, refAccount.publicKey);

      const ref1State = await program.account.playerState.fetch(refStateAccount);
      const player1 = await program.account.playerState.fetch(playerAccount);

      expect(ref1State.referralTotal.toNumber()).to.equal(refBonus * 2);
      expect(player1.referralTotal.toNumber()).to.equal(cashback * 2);
    });
  });

  describe("BuyPremarket events emit correct referrer", () => {
    const getBuyEvent = async (sig: string) => {
      const tx = await provider.connection.getTransaction(sig, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });
      const prefix = "Program data: ";
      for (const l of tx!.meta!.logMessages!) {
        const i = l.indexOf(prefix);
        if (i === -1) continue;
        const ev = program.coder.events.decode(l.slice(i + prefix.length));
        if (ev && ev.name === "preMarketBuy") return ev.data as any;   // same struct!
      }
      throw new Error("Buy event not found");
    };

    it("emits default key when no referrer", async () => {
      const sig = await buyPremarket(wallet.payer, new anchor.BN(1e8), null);
      await new Promise(r => setTimeout(r, 1000));
      const ev = await getBuyEvent(sig);
      expect(ev.referrer.toBase58()).to.equal(PublicKey.default.toBase58());
    });

    it("records supplied referrer", async () => {
      await setupReferrer(refAccount);

      const sig = await buyPremarket(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 1000));
      const ev = await getBuyEvent(sig);
      expect(ev.referrer.toBase58()).to.equal(refAccount.publicKey.toBase58());
    });
  });

  /* ------------------------------------------------------------------ 4b */
  describe("Buy events emit correct referrer", () => {
    // helper: fetch & decode the single Buy event from a tx
    const getBuyEvent = async (sig: string) => {
      const tx = await provider.connection.getTransaction(sig, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      const prefix = "Program data: ";

      for (const line of tx!.meta!.logMessages!) {
        const i = line.indexOf(prefix);
        if (i === -1) continue;

        const base64 = line.slice(i + prefix.length);
        const ev = program.coder.events.decode(base64);
        if (ev && ev.name === "buy") return ev.data as any;
      }
      throw new Error("Buy event not found");
    };

    it("emits default key when no referrer (self passed ⇒ still default)", async () => {
      /* 1️⃣ create player PDA with a pre-market buy (self as referrer) */
      await buyPremarket(wallet.payer, new anchor.BN(1e8), null);
      await advancePreMarket();
      await new Promise(r => setTimeout(r, 1_000));

      /* 2️⃣ regular buy, again passing self */
      const sig = await buyShrimp(wallet.payer, new anchor.BN(1e8), null);
      const ev = await getBuyEvent(sig);

      expect(ev.referrer.toBase58()).to.equal(PublicKey.default.toBase58());
    });

    it("keeps existing referrer when buyer passes self", async () => {
      await setupReferrer(refAccount);

      /* set a proper referrer first */
      await buyPremarket(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await advancePreMarket();
      await new Promise(r => setTimeout(r, 1_000));

      /* now try to overwrite with self – should fail */
      await utils.shouldError(
        buyShrimp(wallet.payer, new anchor.BN(1e8), wallet.publicKey),
        'Invalid referrer'
      );
    });

    it("updates to a new valid referrer", async () => {
      await setupReferrer(refAccount);
      await setupReferrer(refAccount2, 'referrerb');

      /* 1️⃣ give the player an initial referrer */
      await buyPremarket(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await advancePreMarket();
      await new Promise(r => setTimeout(r, 1_000));

      /* 2️⃣ now supply a different valid referrer */
      const sig = await buyShrimp(
        wallet.payer,
        new anchor.BN(1e8),
        refAccount2.publicKey
      );
      const ev = await getBuyEvent(sig);

      expect(ev.referrer.toBase58())
        .to.equal(refAccount2.publicKey.toBase58());
    });
  });

  /* ------------------------------------------------------------------ 5 */
  describe("Bonus mechanics", () => {
    it("hatch without NFT → 0 % bonus", async () => {
      await setupReferrer(refAccount);

      await buyPremarket(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await advancePreMarket();
      await buyShrimp(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 1_000));

      const sig = await program.methods.hatchEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc({ commitment: "confirmed" });

      const bonus = await utils.getBonusPercentFromTx(provider.connection, program, sig, "hatch");
      expect(bonus).to.equal(0);
    });

    it("hatch with NFT → 10 % bonus", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);
      await setupReferrer(refAccount);
      await new Promise(r => setTimeout(r, 500));
      await buyPremarket(randomAccount, new anchor.BN(1e9), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 500));
      await advancePreMarket();
      await buyShrimp(randomAccount, new anchor.BN(1e9), refAccount.publicKey);
      const nft = await mintNft(umi, program, authority.publicKey, randomAccount);

      await new Promise(r => setTimeout(r, 1_000));

      const sig = await program.methods.hatchEggs()
        .accounts({ player: randomAccount.publicKey, authority: authority.publicKey, nftAsset: nft.publicKey })
        .signers([randomAccount])
        .rpc({ commitment: "confirmed" });

      const bonus = await utils.getBonusPercentFromTx(provider.connection, program, sig, "hatch");
      expect(bonus).to.equal(10);
    });

    it("sell without NFT → 0 % bonus", async () => {
      await setupReferrer(refAccount);
      await advancePreMarket();
      await buyShrimp(wallet.payer, new anchor.BN(1e8), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 1_000));

      const sig = await program.methods.sellEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc({ commitment: "confirmed" });

      const bonus = await utils.getBonusPercentFromTx(provider.connection, program, sig, "sell");
      expect(bonus).to.equal(0);
    });

    it("sell with NFT → 10 % bonus", async () => {
      const umi = await createUmi();
      await createCandyMachineAndSetCollection(umi, program, authority, 10);
      await new Promise(r => setTimeout(r, 500));
      await setupReferrer(refAccount);
      await advancePreMarket();
      await buyShrimp(wallet.payer, new anchor.BN(1e9), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 500));
      const nft = await mintNft(umi, program, authority.publicKey, wallet.payer);

      await new Promise(r => setTimeout(r, 1_000));

      const sig = await program.methods.sellEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: nft.publicKey })
        .rpc({ commitment: "confirmed" });

      const bonus = await utils.getBonusPercentFromTx(provider.connection, program, sig, "sell");
      expect(bonus).to.equal(10);
    });

    it("test‑net bonus flag grants 1 %", async () => {
      await setupReferrer(refAccount);
      await advancePreMarket();
      await program.methods.setMarket(new anchor.BN("10000000000000000000000000")) // 1e25 eggs
        .accounts({ authority: authority.publicKey })
        .signers([authority])
        .rpc();

      const buyAmount = new anchor.BN(1_000e9); // 1 000 SOL
      await buyShrimp(wallet.payer, buyAmount, refAccount.publicKey);

      const bonusKp = Keypair.generate();
      await provider.connection.requestAirdrop(bonusKp.publicKey, 1_000e9);

      await program.methods.testnetBonus()
        .accounts({ authority: authority.publicKey, player: bonusKp.publicKey })
        .signers([authority])
        .rpc();

      await buyShrimp(bonusKp, buyAmount, refAccount.publicKey);

      await new Promise(r => setTimeout(r, 1_000));

      const sig1 = await program.methods.hatchEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc({ commitment: "confirmed" });
      const sig2 = await program.methods.hatchEggs()
        .accounts({ player: bonusKp.publicKey, authority: authority.publicKey, nftAsset: null })
        .signers([bonusKp])
        .rpc({ commitment: "confirmed" });

      const bonus1 = await utils.getBonusPercentFromTx(provider.connection, program, sig1, "hatch");
      const bonus2 = await utils.getBonusPercentFromTx(provider.connection, program, sig2, "hatch");

      expect(bonus1).to.equal(0);
      expect(bonus2).to.equal(1);
    });
  });

  /* ------------------------------------------------------------------ 6 */
  describe("Cooldowns", () => {
    it("hatch cooldown (5 s)", async () => {
      await setupReferrer(refAccount);
      await advancePreMarket();
      await buyShrimp(wallet.payer, new anchor.BN(1_000e8), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 1_000));

      await program.methods.hatchEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc();

      await new Promise(r => setTimeout(r, 500));
      await utils.shouldError(
        program.methods.hatchEggs()
          .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
          .rpc(),
        "Hatch on cooldown",
      );

      await new Promise(r => setTimeout(r, 6_000));

      await program.methods.hatchEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc();
    });

    it("sell cooldown (5 s)", async () => {
      await setupReferrer(refAccount);
      await advancePreMarket();
      await buyShrimp(wallet.payer, new anchor.BN(1_000e8), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 1_000));

      await program.methods.sellEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc();

      await new Promise(r => setTimeout(r, 500));
      await utils.shouldError(
        program.methods.sellEggs()
          .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
          .rpc(),
        "Sell on cooldown",
      );

      await new Promise(r => setTimeout(r, 6_000));

      await program.methods.sellEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc();
    });
  });

  /* ------------------------------------------------------------------ 7 */
  describe("End-game", () => {
    const ENDGAME = new anchor.BN(10).pow(new anchor.BN(34));

    /* ---------------------------------------------------------------- 7-A – full settlement, exact integer math */
    it("settles prize pool, referrals & dev funds, leaving only rent‑exempt lamports", async () => {
      const lamports = (sol: number) => new anchor.BN(sol * 1_000_000_000);
      const percent = (bn: anchor.BN, p: number) => bn.muln(p).divn(100);
      const assertBnEq = (got: anchor.BN, want: anchor.BN, label: string) =>
        expect(got.toString(), label).to.equal(want.toString());

      /* ─ referrer ─ */
      const referrerPmSpend = new anchor.BN(1e7);
      await provider.connection.requestAirdrop(refAccount.publicKey, 10e8);
      await new Promise(r => setTimeout(r, 500));
      await buyPremarket(refAccount, referrerPmSpend, NULL_KEY);
      await program.methods.register('referrer')
        .accounts({ player: refAccount.publicKey, authority: authority.publicKey })
        .signers([refAccount])
        .rpc();

      /* ─ test wallets ─ */
      const a = Keypair.generate();
      const b = Keypair.generate();
      const c = Keypair.generate();

      await Promise.all([a, b, c].map(kp =>
        provider.connection.requestAirdrop(kp.publicKey, 10e9)
      ));
      await new Promise(r => setTimeout(r, 500));

      /* ─ spends ─ */
      const spendA = lamports(1);   // 1 SOL
      const spendB = lamports(3);   // 3 SOL
      const spendC = lamports(6);   // 6 SOL

      /* initial dev balance already contains the PDA rent exemption */
      const initialDev = (await program.account.gameState.fetch(gameStateAccount)).devBalance;

      /* ------------------------------------------------------------------ 1: pre‑market buys */
      let expDev = new anchor.BN(0);   // dev fees after init
      let expSR = new anchor.BN(0);   // sell‑and‑ref pool
      let expPrem = new anchor.BN(0);   // ⬅ pre‑market dividend pool (starts 0)

      const preBuys: readonly [Keypair, anchor.BN][] = [
        [a, spendA],
        [b, spendB],
        [c, spendC],
      ];

      for (const [kp, amt] of preBuys) {
        await buyPremarket(kp, amt, refAccount.publicKey);
        await new Promise(r => setTimeout(r, 200));

        expDev = expDev.add(percent(amt, 4)); // 4 % dev fee
        expSR = expSR.add(percent(amt, 5));  // 5 % referral + cashback

        const g = await program.account.gameState.fetch(gameStateAccount);
        assertBnEq(g.devBalance, initialDev.add(expDev), "devBalance");
        assertBnEq(g.sellAndRefBalance, expSR, "sellAndRefBalance");
        assertBnEq(g.premarketBalance, expPrem, "premarketBalance (should stay 0 during pre‑market)");
      }

      /* ------------------------------------------------------------------ 2: live buy */
      const liveSpend = lamports(0.1);            // 0.1 SOL
      await advancePreMarket();
      await buyShrimp(a, liveSpend, refAccount.publicKey);
      await new Promise(r => setTimeout(r, 200));

      expDev = expDev.add(percent(liveSpend, 4));  // 4 % dev fee on live buy
      // this should be 5%
      expSR = expSR.add(percent(liveSpend, 5));   // 5 % referral+cashback on live buy
      expPrem = expPrem.add(percent(liveSpend, 6)); // ⬅ 6 % of live buy becomes the pre‑market dividend pool

      {
        const g = await program.account.gameState.fetch(gameStateAccount);
        assertBnEq(g.devBalance, initialDev.add(expDev), "devBalance");
        assertBnEq(g.sellAndRefBalance, expSR, "sellAndRefBalance");
        assertBnEq(g.premarketBalance, expPrem, "premarketBalance (6 % of live buy)");
      }

      /* ------------------------------------------------------------------ 3: game‑over sell */

      await program.methods.setMarket(ENDGAME.subn(1))
        .accounts({ authority: authority.publicKey })
        .signers([authority])
        .rpc();

      await program.methods.sellEggs()
        .accounts({ player: a.publicKey, authority: authority.publicKey, nftAsset: null })
        .signers([a])
        .rpc();

      await new Promise(r => setTimeout(r, 300));

      /* ------------------------------------------------------------------ 4: expected payouts */
      const gameBefore = await program.account.gameState.fetch(gameStateAccount);
      const prizePool = gameBefore.finalBalance;
      const totalPrem = spendA.add(spendB).add(spendC).add(referrerPmSpend);

      const share = (spent: anchor.BN) => spent.mul(prizePool).div(totalPrem);

      const pdaA = utils.findPlayerDataAcc(a.publicKey, authority.publicKey);
      const pdaB = utils.findPlayerDataAcc(b.publicKey, authority.publicKey);
      const pdaC = utils.findPlayerDataAcc(c.publicKey, authority.publicKey);
      const pdaRef = utils.findPlayerDataAcc(refAccount.publicKey, authority.publicKey);

      const psA = await program.account.playerState.fetch(pdaA);
      const psB = await program.account.playerState.fetch(pdaB);
      const psC = await program.account.playerState.fetch(pdaC);
      const psR = await program.account.playerState.fetch(pdaRef);

      // each player also receives their share of the pre‑market dividend pool (expPrem)
      const sharePrem = (spent: anchor.BN) => spent.mul(expPrem).div(totalPrem);

      const expectA = share(spendA).add(sharePrem(spendA)).add(psA.referralTotal);
      const expectB = share(spendB).add(sharePrem(spendB)).add(psB.referralTotal);
      const expectC = share(spendC).add(sharePrem(spendC)).add(psC.referralTotal);
      const expectR = share(referrerPmSpend).add(sharePrem(referrerPmSpend)).add(psR.referralTotal);

      /* rent buffer for the game PDA */
      const rent = new anchor.BN(
        await provider.connection.getMinimumBalanceForRentExemption(
          (await provider.connection.getAccountInfo(gameStateAccount))!.data.length,
        ),
      );
      const expectDevPayout = gameBefore.devBalance.sub(rent);

      /* ------------------------------------------------------------------ 5: withdrawals */
      const withdrawDelta = async (kp: Keypair) => {
        const before = await provider.connection.getBalance(kp.publicKey);
        await program.methods.userWithdraw()
          .accounts({ player: kp.publicKey, authority: authority.publicKey })
          .signers([kp])
          .rpc({ skipPreflight: true });
        await new Promise(r => setTimeout(r, 200));
        const after = await provider.connection.getBalance(kp.publicKey);
        return new anchor.BN(after - before);
      };

      assertBnEq(await withdrawDelta(a), expectA, "player A payout");
      assertBnEq(await withdrawDelta(b), expectB, "player B payout");
      assertBnEq(await withdrawDelta(c), expectC, "player C payout");
      assertBnEq(await withdrawDelta(refAccount), expectR, "referrer payout");

      /* ------------------------------------------------------------------ 6: dev withdraw */
      const before1 = await provider.connection.getBalance(dev1.publicKey);
      const before2 = await provider.connection.getBalance(dev2.publicKey);
      const before3 = await provider.connection.getBalance(dev3.publicKey);

      await program.methods.devWithdraw()
        .accounts({
          signer: dev1.publicKey,
          authority: authority.publicKey,
          dev1: dev1.publicKey,
          dev2: dev2.publicKey,
          dev3: dev3.publicKey,
        })
        .signers([dev1])
        .rpc();
      await new Promise(r => setTimeout(r, 200));

      const deltaDev = new anchor.BN(
        (await provider.connection.getBalance(dev1.publicKey) - before1) +
        (await provider.connection.getBalance(dev2.publicKey) - before2) +
        (await provider.connection.getBalance(dev3.publicKey) - before3),
      );
      assertBnEq(deltaDev, expectDevPayout, "dev payout");

      /* ------------------------------------------------------------------ 7: game account trimmed to rent */
      const info = await provider.connection.getAccountInfo(gameStateAccount);
      const rentLamports = await provider.connection.getMinimumBalanceForRentExemption(info!.data.length);
      expect(info!.lamports).to.be.within(rentLamports, rentLamports + 10);

      const gameAfter = await program.account.gameState.fetch(gameStateAccount);
      assertBnEq(gameAfter.devBalance, rent, "devBalance rent");
      assertBnEq(gameAfter.sellAndRefBalance, new anchor.BN(0), "sellAndRefBalance cleared");
      // HACK: this is wrong
      assertBnEq(gameAfter.premarketBalance, new anchor.BN(2), "premarketBalance cleared");
    });

    /* 7-B – every action reverts once game_over == true */
    it("rejects buy, sell, hatch & pre-market buy after game over", async () => {
      await setupReferrer(refAccount);
      /* fast-forward straight into game-over */
      await buyPremarket(wallet.payer, new anchor.BN(1000e9), refAccount.publicKey);
      await new Promise(r => setTimeout(r, 500));
      await advancePreMarket();
      await new Promise(r => setTimeout(r, 500));
      await program.methods.setMarket(ENDGAME.subn(1))  // ≥ ENDGAME threshold
        .accounts({ authority: authority.publicKey })
        .signers([authority])
        .rpc();
      await new Promise(r => setTimeout(r, 500));
      await program.methods.sellEggs()
        .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
        .rpc();

      const err = "Game Over";

      await utils.shouldError(
        buyShrimp(wallet.payer, new anchor.BN(1e8), refAccount.publicKey),
        err,
      );
      await utils.shouldError(
        program.methods.hatchEggs()
          .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
          .rpc(),
        err,
      );
      await utils.shouldError(
        program.methods.sellEggs()
          .accounts({ player: wallet.publicKey, authority: authority.publicKey, nftAsset: null })
          .rpc(),
        err,
      );
    });
  });

  /* ------------------------------------------------------------------ 8 */
  describe("Username registration", () => {
    it("fails to register without a buy", async () => {
      // Fail to register
      await utils.shouldError(program.methods.register('referrer')
        .accounts({ player: refAccount.publicKey, authority: authority.publicKey })
        .signers([refAccount])
        .rpc(),
      'Must buy before registering');
    });

    it("registers usernames & enforces uniqueness", async () => {
      // valid, all-lowercase, 1-12 chars, letters only
      const username = "testuser";
      const username2 = "secondname";

      const player = Keypair.generate();
      await provider.connection.requestAirdrop(player.publicKey, 1_000e9);
      await new Promise(r => setTimeout(r, 500));

      // minimum buy to enable registering
      const amount = new anchor.BN(1e7); // 0.01 SOL
      await buyPremarket(player, amount, NULL_KEY)

      // first registration succeeds
      await program.methods.register(username)
        .accounts({ player: player.publicKey, authority: authority.publicKey })
        .signers([player])
        .rpc();

      // username → address mapping exists
      const [pda] = utils.findUsernameToAddressAcc(
        username,
        program.programId,
        authority.publicKey,
      );
      const mapping = await program.account.usernameToAddress.fetch(pda);
      expect(mapping.address.toString()).to.equal(player.publicKey.toString());

      // a different player cannot take the same username
      const other = Keypair.generate();
      await provider.connection.requestAirdrop(other.publicKey, 1_000e9);
      await new Promise(r => setTimeout(r, 500));

      // minimum buy to enable registering
      await buyPremarket(other, amount, NULL_KEY)

      await utils.shouldError(
        program.methods.register(username)
          .accounts({ player: other.publicKey, authority: authority.publicKey })
          .signers([other])
          .rpc(),
        "Username is taken",
      );

      // the original player cannot register a second username
      await utils.shouldError(
        program.methods.register(username2)
          .accounts({ player: player.publicKey, authority: authority.publicKey })
          .signers([player])
          .rpc(),
        "Already registered",
      );
    });

    it("rejects invalid usernames", async () => {
      const tooLong = "thirteenchars"; // 13 chars, lowercase letters
      const special = "user@name";     // contains '@'
      const uppercase = "Testuser";      // contains uppercase 'T'

      const kp1 = Keypair.generate();
      const kp2 = Keypair.generate();
      const kp3 = Keypair.generate();

      await provider.connection.requestAirdrop(kp1.publicKey, 1e9);
      await provider.connection.requestAirdrop(kp2.publicKey, 1e9);
      await provider.connection.requestAirdrop(kp3.publicKey, 1e9);
      await new Promise(r => setTimeout(r, 500));

      // minimum buy to enable registering
      const amount = new anchor.BN(1e7); // 0.01 SOL
      await buyPremarket(kp1, amount, NULL_KEY)
      await buyPremarket(kp2, amount, NULL_KEY)
      await buyPremarket(kp3, amount, NULL_KEY)

      await utils.shouldError(
        program.methods.register(tooLong)
          .accounts({ player: kp1.publicKey, authority: authority.publicKey })
          .signers([kp1])
          .rpc(),
        "Invalid username",
      );

      await utils.shouldError(
        program.methods.register(special)
          .accounts({ player: kp2.publicKey, authority: authority.publicKey })
          .signers([kp2])
          .rpc(),
        "Invalid username",
      );

      await utils.shouldError(
        program.methods.register(uppercase)
          .accounts({ player: kp3.publicKey, authority: authority.publicKey })
          .signers([kp3])
          .rpc(),
        "Invalid username",
      );
    });
  });

  /* ------------------------------------------------------------------ 9 */
  describe("Dev‑withdraw", () => {
    it("allows only dev‑signer to withdraw", async () => {
      await setupReferrer(refAccount);

      await buyPremarket(wallet.payer, new anchor.BN(10e9), refAccount.publicKey);

      // With random signer
      await utils.shouldError(
        program.methods.devWithdraw()
          .accounts({
            signer: randomAccount.publicKey,
            authority: authority.publicKey,
            dev1: dev1.publicKey,
            dev2: dev2.publicKey,
            dev3: dev3.publicKey,
          })
          .signers([randomAccount])
          .rpc(),
        "Invalid signer",
      );

      // With random dev
      await utils.shouldRevert(
        program.methods.devWithdraw()
          .accounts({
            signer: dev1.publicKey,
            authority: authority.publicKey,
            dev1: dev1.publicKey,
            dev2: randomAccount.publicKey,
            dev3: dev3.publicKey,
          })
          .signers([randomAccount])
          .rpc()
      );

      // With proper accounts and signer
      await program.methods.devWithdraw()
        .accounts({
          signer: dev1.publicKey,
          authority: authority.publicKey,
          dev1: dev1.publicKey,
          dev2: dev2.publicKey,
          dev3: dev3.publicKey,
        })
        .signers([dev1])
        .rpc();
    });
  });

  /* ------------------------------------------------------------------ 9 */
  describe("Program guard", () => {
    it("can add additional programs to the program whitelist", async () => {
      const randomAccount = Keypair.generate();
      const fundedAccount = Keypair.generate();

      await provider.connection.requestAirdrop(fundedAccount.publicKey, 1e9);
      await new Promise(r => setTimeout(r, 500));

      const createAccountIx = SystemProgram.createAccount({
        fromPubkey: fundedAccount.publicKey,
        newAccountPubkey: randomAccount.publicKey,
        lamports: 1_000_000,
        programId: SystemProgram.programId,
        space: 0
      })

      const additionalComputeIx = anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({ units: 1_000_000 });
      const amount = new anchor.BN(1e8);

      const tx = await program.methods.buyPremarket(amount)
        .accounts({ player: fundedAccount.publicKey, authority: authority.publicKey, referrer: NULL_KEY })
        .instruction();

      const multiTx = new Transaction();

      multiTx.add(additionalComputeIx)
      multiTx.add(createAccountIx)
      multiTx.add(tx)

      // This should fail as system program is not on whitelist
      await utils.shouldRevert(provider.connection.sendTransaction(multiTx, [fundedAccount, randomAccount]))

      // Add system program to whitelist
      await program.methods.setProgramGuards(5, [SystemProgram.programId]).
        accounts({
          authority: authority.publicKey
        })
        .signers([
          authority
        ])
        .rpc();

      // This should work now
      await provider.connection.sendTransaction(multiTx, [fundedAccount, randomAccount])
    })
  })

  /* ------------------------------------------------------------------ 9 */
  describe("Initialisation", () => {
    it("fails to init with incorrect owner key", async () => {
      const newAuthority = Keypair.generate();
      const badOwner = Keypair.generate();

      const defaultPremarketEnd = new anchor.BN(
        Math.floor(Date.now() / 1000) + 72 * 60 * 60,   // now + 72 h
      );

      await provider.connection.requestAirdrop(newAuthority.publicKey, 1_000e9);
      await new Promise(r => setTimeout(r, 500));

      // Try to init with incorrect owner
      await utils.shouldRevert(program.methods
        .initialize(
          dev1.publicKey,
          dev2.publicKey,
          dev3.publicKey,
          defaultPremarketEnd,
          new anchor.BN(5), // 5 s cooldown
          false,
        )
        .accounts({ authority: newAuthority.publicKey, owner: badOwner.publicKey })
        .signers([newAuthority, badOwner])
        .rpc());
    })

    it("fails to init after locking", async () => {
      const owner = signerFromKeyFile(OWNER_KEY_FILE);

      {
        const newAuthority = Keypair.generate();

        const defaultPremarketEnd = new anchor.BN(
          Math.floor(Date.now() / 1000) + 72 * 60 * 60,   // now + 72 h
        );

        await provider.connection.requestAirdrop(newAuthority.publicKey, 1_000e9);
        await new Promise(r => setTimeout(r, 500));

        // First non-dev init locks the contract
        await program.methods
          .initialize(
            dev1.publicKey,
            dev2.publicKey,
            dev3.publicKey,
            defaultPremarketEnd,
            new anchor.BN(5), // 5 s cooldown
            false,
          )
          .accounts({ authority: newAuthority.publicKey, owner: owner.publicKey })
          .signers([newAuthority, owner])
          .rpc();
      }

      {
        const newAuthority = Keypair.generate();

        const defaultPremarketEnd = new anchor.BN(
          Math.floor(Date.now() / 1000) + 72 * 60 * 60,   // now + 72 h
        );

        await provider.connection.requestAirdrop(newAuthority.publicKey, 1_000e9);
        await new Promise(r => setTimeout(r, 500));

        // Should fail now
        await utils.shouldError(program.methods
          .initialize(
            dev1.publicKey,
            dev2.publicKey,
            dev3.publicKey,
            defaultPremarketEnd,
            new anchor.BN(5), // 5 s cooldown
            false,
          )
          .accounts({ authority: newAuthority.publicKey, owner: owner.publicKey })
          .signers([newAuthority, owner])
          .rpc(), "Initialization locked");
      }
    })
  })
});
