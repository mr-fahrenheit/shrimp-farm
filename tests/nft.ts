import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Keypair, PublicKey } from "@solana/web3.js";
import { publicKey as metaplexPublicKey } from '@metaplex-foundation/umi';

import {
    createCandyMachine,
    addConfigLines,
    findCandyMachineAuthorityPda,
} from '@metaplex-foundation/mpl-core-candy-machine';

import {
    generateSigner,
    some,
    TransactionBuilderSendAndConfirmOptions,
    Umi,
    KeypairSigner,
} from "@metaplex-foundation/umi";

const { ComputeBudgetProgram } = anchor.web3;
import { createCollection, ruleSet } from '@metaplex-foundation/mpl-core';
import { Shrimp } from '../target/types/shrimp';
import { findGameDataAcc, findPlayerDataAcc } from "./utils";

// Common transaction builder options for convenience
const options: TransactionBuilderSendAndConfirmOptions = {
    send: { skipPreflight: true },
    confirm: { commitment: 'processed' },
};

export async function createCandyMachineAndSetCollection(umi: Umi, shrimpProgram: Program<Shrimp>, authority: Keypair, totalItems = 1024): Promise<{ collection: KeypairSigner, candyMachine: KeypairSigner }> {
    const collection = generateSigner(umi);
    const candyMachine = generateSigner(umi);

    return createCandyMachineAndSetCollectionWithKeys(umi, shrimpProgram, authority,collection, candyMachine, totalItems);
}

/**
 * Creates a Candy Machine & a Collection NFT using Metaplex,
 * then sets them in the shrimp game via `setCollection` & `setMintAuthority`.
 */
export async function createCandyMachineAndSetCollectionWithKeys(umi: Umi, shrimpProgram: Program<Shrimp>, authority: Keypair, collection: KeypairSigner, candyMachine: KeypairSigner, totalItems = 1024): Promise<{ collection: KeypairSigner, candyMachine: KeypairSigner }> {
    // Create a Collection.
    try {
        await createCollection(umi, {
            collection: collection,   // ← required signer
            name: 'Shrimp Farm',
            uri: 'https://arweave.net/y07lP84vWYNKLDRGuLN9Q6r8S9oHUOze2HMchkPxy-A',
            plugins: [
              {
                type: 'Royalties',
                basisPoints: 300,
                creators: [
                  {
                    address: metaplexPublicKey("MrFFFfy8qifTYGvPZAvWr9Tfkpi32u7ZQn9dZfA4C1o"),
                    percentage: 100,
                  },
                ],
                ruleSet: ruleSet('None'), // Compatibility rule set
              },
            ],
          }).sendAndConfirm(umi);
    } catch (error) {
        //console.log('Error creating collection.');
        throw error;
    }

    // Create a Candy Machine.
    try {
        const createIx = await createCandyMachine(umi, {
            candyMachine,
            collection: collection.publicKey,
            collectionUpdateAuthority: umi.identity,
            itemsAvailable: totalItems,
            authority: umi.identity.publicKey,
            isMutable: true,
            configLineSettings: some({
                prefixName: 'Farmer #',
                nameLength: 4,
                prefixUri: 'https://arweave.net/SoyEoRPeCPcvlO_2SQ3NL4mpX8VLOGTXESuq4wT5MRY/',
                uriLength: 9,
                isSequential: false,
            }),
        });
        await createIx.sendAndConfirm(umi, options);
    } catch (error) {
        console.log('Error creating Candy Machine.');
        throw error;
    }

    const batchSize = 40; // adjust this value as needed

    for (let i = 0; i < totalItems; i += batchSize) {
        // Create a batch of config lines
        const batch = [];
        for (let j = i + 1; j <= Math.min(i + batchSize, totalItems); j++) {
            batch.push({ name: `${j}`, uri: `${j}.json` });
        }
        try {
            await addConfigLines(umi, {
                candyMachine: candyMachine.publicKey,
                index: i, // starting index for this batch
                configLines: batch,
            })
            .sendAndConfirm(umi, options);
        } catch (error) {
            console.log(`Error adding batch starting at index ${i}.`);
            throw error;
        }
    }

    // Set collection on shrimp program
    try {
        await shrimpProgram.methods
            .setCollection()
            .accounts({
                authority: authority.publicKey,
                candyMachine: candyMachine.publicKey,
                candyMachineAuthority: new PublicKey(umi.payer.publicKey.toString()),
            })
            .signers([authority])
            .rpc({ commitment: 'confirmed' });
    } catch (error) {
        //console.log('Error setting collection.');
        throw error;
    }

    return {
        collection,
        candyMachine,
    };
}

export async function mintNft(umi: Umi, shrimpProgram: Program<Shrimp>, authority: PublicKey, player: Keypair): Promise<Keypair> {
    const nftMintSigner = anchor.web3.Keypair.generate();

    const gameStateAccount = await findGameDataAcc(authority);
    const playerAccount = await findPlayerDataAcc(player.publicKey, authority)
    const gameState = await shrimpProgram.account.gameState.fetch(gameStateAccount);
    const candyMachine = gameState.candymachineKey;
    const collection = gameState.collectionKey;

    const authorityPda = findCandyMachineAuthorityPda(umi, { candyMachine: metaplexPublicKey(candyMachine.toString())});
    const computeIx = ComputeBudgetProgram.setComputeUnitLimit({ units: 9000000 });

    await shrimpProgram.methods
        .mintNft()
        .accountsPartial({
            player: player.publicKey,
            authority: authority,
            candyMachine: candyMachine,
            authorityPda: authorityPda[0],
            asset: nftMintSigner.publicKey,
            collection: collection,
        })
        .preInstructions([computeIx])
        .signers([nftMintSigner, player])
        .rpc({ commitment: "confirmed", skipPreflight: true });

    return nftMintSigner;
}

export async function adminMint(
  umi: Umi,
  shrimpProgram: Program<Shrimp>,
  authority: PublicKey,     // game-authority (same one used everywhere else)
  admin: Keypair,           // MUST be the hard-coded owner key
  player: Keypair,          // wallet that will receive the NFT
): Promise<Keypair> {

 const [minterStateAccount] = PublicKey.findProgramAddressSync(
    [Buffer.from("minter"), authority.toBuffer()],
    shrimpProgram.programId
  );

  const assetMint = anchor.web3.Keypair.generate();          // fresh mint

  const gameStateAccount = await findGameDataAcc(authority);
  const gameState        = await shrimpProgram.account.gameState.fetch(gameStateAccount);

  const candyMachine = gameState.candymachineKey;
  const collection   = gameState.collectionKey;

  // PDA the candy-machine expects as its authority
  const authorityPda = findCandyMachineAuthorityPda(
    umi,
    { candyMachine: metaplexPublicKey(candyMachine.toString()) },
  );

  // A generous CU budget – same as the regular mintNft helper
  const computeIx = ComputeBudgetProgram.setComputeUnitLimit({ units: 9_000_000 });

  await shrimpProgram.methods
    .adminMint()                               // ← new on-chain instruction
    .accountsPartial({
      admin:        admin.publicKey,
      player:       player.publicKey,
      authority,                               // game authority
      minterState: minterStateAccount,         // minter state
      candyMachine,                            // candy-machine
      authorityPda: authorityPda[0],           // PDA needed by the CPI
      asset:        assetMint.publicKey,       // new mint
      collection: collection,                  // collection mint
    })
    .preInstructions([computeIx])
    .signers([assetMint, admin])               // asset + admin must sign
    .rpc({ commitment: "confirmed", skipPreflight:false });

  return assetMint;                            // return the newly-created mint
}