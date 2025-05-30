use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    sysvar
};
use anchor_lang::solana_program::clock;
use crate::state::{GameState, PlayerState, CANDY_MACHINE_AUTHORITY_SEED, NFT_MIN_BUY};

/// The mint asset discriminator for candy machine
const MINT_DISCRIMINATOR: [u8; 12] = [
    84, 175, 211, 156, 56,
   250, 104, 118,   0,  0,
     0,   0
 ];

#[derive(Accounts)]
pub struct MintNft<'info> {
    /// Signer paying for new PDA creation & transaction fees
    #[account(mut)]
    pub player: Signer<'info>,

    /// CHECK: Only valid authority will be able to execute mint CPI
    pub authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [player.key().as_ref(), PlayerState::SEED, authority.key().as_ref()],
        bump
    )]
    pub player_state: Account<'info, PlayerState>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Account<'info, GameState>,

    // --------------------------------------------------
    //  Candy Machine CPI accounts below
    // --------------------------------------------------

    /// Candy machine program
    /// CHECK: Custom address constraint
    #[account(address = pubkey!("CMACYFENjoBMHzapRXyo1JZkVS6EtaDDzkjMrmQLvr4J"))]
    pub candy_machine_program: AccountInfo<'info>,

    /// Candy machine account
    /// CHECK: Custom address constraint
    #[account(mut, address = game_state.candymachine_key)]
    pub candy_machine: UncheckedAccount<'info>,

    /// Candy Machine authority account.
    /// CHECK: Account constraints checked in candy machine CPI
    #[account(mut)]
    pub authority_pda: UncheckedAccount<'info>,

    /// CHECK: The “mint_authority” PDA
    #[account(seeds = [CANDY_MACHINE_AUTHORITY_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub mint_authority: AccountInfo<'info>,

    /// Mint account of the NFT - will be initialized if necessary
    /// CHECK: Account checked in CPI
    #[account(mut)]
    asset: Signer<'info>,

    /// Mint account of the collection NFT
    /// CHECK: Account checked in CPI
    #[account(mut)]
    collection: UncheckedAccount<'info>,

    /// MPL Core program
    /// CHECK: Account checked in CPI
    #[account(address = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"))]
    mpl_core_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,

    /// Instructions sysvar account
    /// CHECK: Account constraint checked in account trait
    #[account(address = sysvar::instructions::id())]
    pub sysvar_instructions: UncheckedAccount<'info>,

    /// SlotHashes sysvar cluster data
    /// CHECK: Account constraint checked in account trait
    #[account(address = sysvar::slot_hashes::id())]
    pub recent_slothashes: UncheckedAccount<'info>,
}

pub fn mint_nft(ctx: Context<MintNft>) -> Result<()> {
    // Get state
    let player_state = &mut ctx.accounts.player_state;
    let game_state = &mut ctx.accounts.game_state;

    // Return if NFT is already minted or player hasn’t spent enough
    if player_state.minted 
       || (player_state.market_spent + player_state.premarket_spent) < NFT_MIN_BUY 
    {
        return Ok(());
    }

    // Return if all NFTs minted
    if game_state.nfts_minted == 1024 {
        return Ok(());
    }

    // Mark as minted to prevent duplicate mints
    player_state.minted = true;

    // Construct required accounts and instruction data for the Candy Machine CPI
    let account_metas = vec![
        AccountMeta::new(ctx.accounts.candy_machine.key(), false),
        AccountMeta::new(ctx.accounts.authority_pda.key(), false),
        AccountMeta::new_readonly(ctx.accounts.mint_authority.key(), true),
        AccountMeta::new(ctx.accounts.player.key(), true),
        AccountMeta::new_readonly(ctx.accounts.player.key(), false),
        AccountMeta::new(ctx.accounts.asset.key(), true),
        AccountMeta::new(ctx.accounts.collection.key(), false),
        AccountMeta::new_readonly(ctx.accounts.mpl_core_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.sysvar_instructions.key(), false),
        AccountMeta::new_readonly(ctx.accounts.recent_slothashes.key(), false),
    ];

    let mint_accounts = vec![
        ctx.accounts.candy_machine.clone().to_account_info(),
        ctx.accounts.authority_pda.clone().to_account_info(),
        ctx.accounts.mint_authority.to_account_info(),
        ctx.accounts.player.clone().to_account_info(),
        ctx.accounts.player.clone().to_account_info(),
        ctx.accounts.asset.clone().to_account_info(),
        ctx.accounts.collection.clone().to_account_info(),
        ctx.accounts.mpl_core_program.clone().to_account_info(),
        ctx.accounts.system_program.clone().to_account_info(),
        ctx.accounts.sysvar_instructions.clone().to_account_info(),
        ctx.accounts.recent_slothashes.clone().to_account_info(),
    ];

    // Construct the instruction data using the discriminator
    let ix_data = MINT_DISCRIMINATOR.to_vec();
    let ix = Instruction {
        program_id: ctx.accounts.candy_machine_program.key(),
        accounts: account_metas.clone(),
        data: ix_data,
    };

    // Invoke the Candy Machine mint instruction using the PDA signer
    invoke_signed(
        &ix, 
        &mint_accounts, 
        &[&[
            CANDY_MACHINE_AUTHORITY_SEED.as_bytes(),
            &ctx.accounts.authority.key().to_bytes(),
            &[ctx.bumps.mint_authority],
        ]]
    )?;

    // Increment the global NFTs minted counter after a successful mint
    game_state.nfts_minted += 1;

    // Emit an event with the updated total supply (nfts_minted)
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    emit!(NftMinted {
        event_index: game_state.event_index,
        player: ctx.accounts.player.key(),
        nfts_minted: game_state.nfts_minted,
        timestamp: now,
    });

    // Update indexes
    game_state.event_index = game_state.event_index.checked_add(1).unwrap();

    Ok(())
}


#[event]
pub struct NftMinted {
    pub event_index: u64,
    pub player: Pubkey,
    pub nfts_minted: u16,
    pub timestamp: u64,
}