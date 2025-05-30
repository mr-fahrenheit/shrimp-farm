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
    const txs = [];

    for (let i = 0; i < totalItems; i += batchSize) {
        // Create a batch of config lines
        const batch = [];
        for (let j = i + 1; j <= Math.min(i + batchSize, totalItems); j++) {
            batch.push({ name: `${j}`, uri: `${j}.json` });
        }
        txs.push(addConfigLines(umi, {
            candyMachine: candyMachine.publicKey,
            index: i, // starting index for this batch
            configLines: batch,
        }))
    }

    for (let i = 0; i < txs.length; i++) {
        const tx = txs[i];
        do
        {
            try {
                await new Promise(r => setTimeout(r, 5000));
                await tx.sendAndConfirm(umi, options);
                break;
            } catch (error) {
                console.error(error);
                console.log(`Error adding batch starting at index ${i}. Retrying.`);
            }
        } while( true );
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