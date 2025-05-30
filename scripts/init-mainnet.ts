import * as fs from 'fs';
import * as anchor from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { Shrimp } from "../target/types/shrimp";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { createCandyMachineAndSetCollectionWithKeys } from "../tests/nft";
import { createSignerFromKeypair, signerIdentity } from "@metaplex-foundation/umi";
import { mplCandyMachine as mplCoreCandyMachine } from '@metaplex-foundation/mpl-core-candy-machine';
import { DateTime } from "luxon"; 

function nextMondayNoonUtc(): number {
  // Grab "now" in the New-York zone (handles EST/EDT for you)
  const nowNy = DateTime.now().setZone("America/New_York");

  // Days until Monday (0 = today if it’s Monday, otherwise 1-6)
  let daysToMon = (8 - nowNy.weekday) % 7;

  // If it *is* Monday but already past 12:00, jump to next week
  if (daysToMon === 0 && nowNy.hour >= 12) {
    daysToMon = 7;
  }

  // Build the next-Monday-at-noon DateTime in NY
  const noonNy = nowNy
    .plus({ days: daysToMon })
    .startOf("day")
    .plus({ hours: 12 });

  // Convert to UTC seconds
  return Math.floor(noonNy.toUTC().toSeconds());
}

const PREMARKET_END_TS = nextMondayNoonUtc();
const COOLDOWN_SECS    = 60 * 60 * 8;                         // 8 hr

const DEV1_ADDRESS = "MrFFFfy8qifTYGvPZAvWr9Tfkpi32u7ZQn9dZfA4C1o";
const DEV2_ADDRESS = "3Xj6iMSaq2gurqJ6pPR6vbYPn4wy1NsB8g39tqmjwGzW";
const DEV3_ADDRESS = "44KzFGFkeBdYS9XZcKioCyXpuiDGjy4sRhNufx46EeWS";

const AUTHORITY_KEY_FILE = "keypairs/prod/authority.json";
const COLLECTION_KEY_FILE = "keypairs/prod/collection.json";
const CANDYMACHINE_KEY_FILE = "keypairs/prod/candymachine.json";
const OWNER_KEY_FILE = "keypairs/owner.json";

function walletFromKeyFile(umi, keyfile) {
  const data = fs.readFileSync(keyfile);
  const jsonData = JSON.parse(data.toString('utf-8'));
  return createSignerFromKeypair(umi, umi.eddsa.createKeypairFromSecretKey(new Uint8Array(jsonData)));
}

function signerFromKeyFile(keyfile) {
  const data = fs.readFileSync(keyfile);
  const jsonData = JSON.parse(data.toString('utf-8'));
  return Keypair.fromSecretKey(new Uint8Array(jsonData));
}

async function deploy() {
  // Setup anchor and program
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Shrimp as anchor.Program<Shrimp>;

  // Create UMI for interacting with Metaplex
  const umi = await createUmi(provider.connection.rpcEndpoint, undefined);

  // Load keys
  const signer = walletFromKeyFile(umi, AUTHORITY_KEY_FILE);
  const collection = walletFromKeyFile(umi, COLLECTION_KEY_FILE)
  const candyMachine = walletFromKeyFile(umi, CANDYMACHINE_KEY_FILE);
  const authority = signerFromKeyFile(AUTHORITY_KEY_FILE);
  const owner = signerFromKeyFile(OWNER_KEY_FILE);

  // Configure UMI
  umi.use(signerIdentity(signer, true));
  umi.use(mplCoreCandyMachine());

  // Output info
  console.log("Creating new game");
  console.log("Program:    ", program.programId.toBase58());
  console.log("Authority:  ", authority.publicKey.toBase58());
  console.log(
    "Premarket ends:", new Date(PREMARKET_END_TS * 1000).toISOString(),
  );
  console.log(`Cooldown: ${COOLDOWN_SECS} s`);

  // Initialize the Shrimp game
  await program.methods
    .initialize(
      new PublicKey(DEV1_ADDRESS),
      new PublicKey(DEV2_ADDRESS),
      new PublicKey(DEV3_ADDRESS),
      new anchor.BN(PREMARKET_END_TS),
      new anchor.BN(COOLDOWN_SECS),
      false
    )
    .accounts({
      authority: authority.publicKey,
      owner: owner.publicKey,
    })
    .signers([authority, owner])
    .rpc(); 

  // Create Candy Machine and set the collection
  await createCandyMachineAndSetCollectionWithKeys(umi, program, authority, collection, candyMachine);

  // Output info
  console.log("Candy Machine:", candyMachine.publicKey.toString());
  console.log("Collection:", collection.publicKey.toString());
  console.log();
  console.log("Initialization complete!");
}

deploy();
