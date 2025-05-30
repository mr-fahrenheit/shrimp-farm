import { PublicKey, Connection } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { SHRIMP_PROGRAM_ID } from "./constants";
import { assert, expect } from 'chai';

const TOKEN_METADATA_PROGRAM_ID = new anchor.web3.PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

async function getPDAPublicKey(seeds: Array<Buffer | Uint8Array>, programId: PublicKey) {
  return (await getPDA(seeds, programId))[0];
}

function getPDA(seeds: Array<Buffer | Uint8Array>, programId: PublicKey) {
  return PublicKey.findProgramAddressSync(
    seeds,
    programId
);
}

const getMetadata = async (mint: anchor.web3.PublicKey): Promise<anchor.web3.PublicKey> => {
  return await getPDAPublicKey(
    [Buffer.from("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), mint.toBuffer()],
    TOKEN_METADATA_PROGRAM_ID,
  );
};

const getMasterEdition = async (mint: anchor.web3.PublicKey): Promise<anchor.web3.PublicKey> => {
  return await getPDAPublicKey(
    [Buffer.from("metadata"), TOKEN_METADATA_PROGRAM_ID.toBuffer(), mint.toBuffer(), Buffer.from("edition")],
    TOKEN_METADATA_PROGRAM_ID,
  );
};

const findPlayerDataAcc = (player: PublicKey, authority: PublicKey) => {
  const [playerDataAcc, _] = PublicKey.findProgramAddressSync(
      [player.toBuffer(), Buffer.from("shrimp"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );    
  return playerDataAcc;
};

const findContractDataAcc = (authority: PublicKey) => {
  const [assetManager] = PublicKey.findProgramAddressSync(
      [Buffer.from("contractdata"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );
  return assetManager
}

const findGameDataAcc = (authority: PublicKey) => {
  const [assetManager] = PublicKey.findProgramAddressSync(
      [Buffer.from("shrimp"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );
  return assetManager
}


const findGameTreasuryAcc = (authority: PublicKey)=> {
  const [assetManager] = PublicKey.findProgramAddressSync(
      [Buffer.from("treasury"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );
  return assetManager
}

const findNftMintAuthority = (authority: PublicKey)=> {
  const [assetManager] = PublicKey.findProgramAddressSync(
      [Buffer.from("candy_machine"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );
  return assetManager
}

const findUsernameToAddressAcc = (username: string, programId: PublicKey, authority: PublicKey): [PublicKey, number] => {
  return PublicKey.findProgramAddressSync([Buffer.from("username_to_address"), Buffer.from(username), authority.toBuffer()], programId);
};

const findAddressToUsernameAcc = (playerPubKey, programId, authority) => {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("address_to_username"), 
      playerPubKey.toBuffer(),
      authority.toBuffer()
    ],
    programId
  )
};

const findPlayerDataAccWithDebug = (player: PublicKey, authority: PublicKey) => {
  console.log("Debug PDA derivation:");
  console.log("Player pubkey bytes:", [...player.toBuffer()]);
  console.log("Authority pubkey bytes:", [...authority.toBuffer()]);
  
  const [playerDataAcc, bump] = PublicKey.findProgramAddressSync(
      [player.toBuffer(), Buffer.from("shrimp"), authority.toBuffer()],
      SHRIMP_PROGRAM_ID
  );
  
  console.log("Generated PDA:", playerDataAcc.toString());
  console.log("Bump:", bump);
  
  return playerDataAcc;
};

async function shouldError(promise, expectedErrorMessage) {
    try {
        await promise;
        assert(false);
    }
    catch(err) {
      expect(err).to.be.instanceOf(anchor.AnchorError);
      expect((err as anchor.AnchorError).error.errorMessage).to.equal(expectedErrorMessage);
    }
}

async function shouldRevert(promise) {
    try {
        await promise;
        assert(false);
    }
    catch(err) {}
}

async function getBonusPercentFromTx(
  connection: Connection,
  program:   anchor.Program<any>,
  txSig:     string,
  eventName: "sell" | "hatch",
): Promise<number | null> {

  const tx = await connection.getTransaction(txSig, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });
  if (!tx?.meta?.logMessages) return null;

  const parser = new anchor.EventParser(program.programId, program.coder);

  for (const evt of parser.parseLogs(tx.meta.logMessages)) {
    if (evt.name === eventName) {
      return (evt.data as any).bonusPercent as number;
    }
  }
  return null;                 // event not found
}


// Types

// type TFinalized = { authority: PublicKey };
// type TInitialized = { owner: PublicKey, totalSupply: Number, seedBalance: Number, mrfWallet: PublicKey, bkWallet: PublicKey, shiragaWallet: PublicKey };
// type TNFTMinted = { id: PublicKey, holder: PublicKey, uri: String };
// type TFeeUpdated = { fee: Number };
// type TDevWithdrawn = { owner: PublicKey, devBalance: Number; mrfAmount: Number, bkAmount: Number, shiragaAmount:Number, authority: PublicKey };
// type TUserWithdrawn = { owner: PublicKey, amount: Number; lastInteraction: Number, marketEggs: Number, sellTotal: Number };
// type TBuy = { owner: PublicKey, shrimpToAdd: Number, refFee: Number, refAddress: PublicKey, extraEggs: Number, premarketEarned:Number, devBalance: Number, amount: Number; lastInteraction: Number, shrimp: Number, marketEggs: Number };
// type TSell= { owner: PublicKey, eggSell: Number, sellTotal: Number, nftHolder:Boolean, eggs: Number; lastInteraction: Number,  marketEggs: Number};
// type THatch = { owner: PublicKey, shrimpToAdd: Number, nftHolder: Boolean, eggs: Number; lastInteraction: Number,  shrimp: Number};
// type TBuyPremarket = { owner: PublicKey, refFee: Number, refAddress: PublicKey, premarketSpent:Number, gamePremarketSpent:Number, amount: Number; lastInteraction: Number, shrimp: Number, marketEggs: Number };
// type TMarketUpdated = { newMarketEggs: anchor.BN };

export { TOKEN_METADATA_PROGRAM_ID,
  findPlayerDataAcc, findGameDataAcc, findContractDataAcc, findGameTreasuryAcc, findNftMintAuthority, findUsernameToAddressAcc, findAddressToUsernameAcc, shouldError, shouldRevert, findPlayerDataAccWithDebug,
  getPDAPublicKey, getMetadata, getMasterEdition, getBonusPercentFromTx };
