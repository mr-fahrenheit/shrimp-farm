
use crate::{error::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    sysvar
};
use anchor_lang::solana_program::clock;
use crate::state::{GameState, MinterState, CANDY_MACHINE_AUTHORITY_SEED};

/// The mint asset discriminator for candy machine
const MINT_DISCRIMINATOR: [u8; 12] = [
    84, 175, 211, 156, 56,
   250, 104, 118,   0,  0,
     0,   0,
];

/// Admin-only mint; identical CPI to `MintNft` but without `PlayerState`
#[derive(Accounts)]
pub struct AdminMint<'info> {
    /// Signer paying for new PDA creation & transaction fees
    #[account(mut, address = minter_state.minter)]
    pub admin: Signer<'info>,

    /// CHECK: Only valid authority will be able to execute mint CPI
    pub authority: AccountInfo<'info>,
    
    #[account(
        seeds = [MinterState::SEED, authority.key().as_ref()],
        bump
    )]
    pub minter_state: Account<'info, MinterState>,

    #[account(
        mut,
        seeds = [GameState::SEED, authority.key().as_ref()],
        bump
    )]
    pub game_state: Account<'info, GameState>,

    /// CHECK: Wallet that will receive the NFT
    pub player: UncheckedAccount<'info>,

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

    /// CHECK: The "mint_authority" PDA
    #[account(seeds = [CANDY_MACHINE_AUTHORITY_SEED.as_bytes(), authority.key().as_ref()], bump)]
    pub mint_authority: AccountInfo<'info>,

    /// Mint account of the NFT – will be initialized if necessary
    /// CHECK: Account checked in CPI
    #[account(mut)]
    pub asset: Signer<'info>,

    /// Mint account of the collection NFT
    /// CHECK: Account checked in CPI
    #[account(mut)]
    pub collection: UncheckedAccount<'info>,

    /// MPL Core program
    /// CHECK: Account checked in CPI
    #[account(address = pubkey!("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d"))]
    pub mpl_core_program: UncheckedAccount<'info>,

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

pub fn admin_mint(ctx: Context<AdminMint>) -> Result<()> {
    // Check if we are minted out
    require!(ctx.accounts.game_state.nfts_minted < 1_024, CustomErrors::MintedOut);

    // --------------------------------------------------
    //  Build Candy Machine CPI
    // --------------------------------------------------
    let account_metas = vec![
        AccountMeta::new(ctx.accounts.candy_machine.key(), false),
        AccountMeta::new(ctx.accounts.authority_pda.key(), false),
        AccountMeta::new_readonly(ctx.accounts.mint_authority.key(), true),
        AccountMeta::new(ctx.accounts.admin.key(), true),
        AccountMeta::new_readonly(ctx.accounts.player.key(), false),
        AccountMeta::new(ctx.accounts.asset.key(), true),
        AccountMeta::new(ctx.accounts.collection.key(), false),
        AccountMeta::new_readonly(ctx.accounts.mpl_core_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.system_program.key(), false),
        AccountMeta::new_readonly(ctx.accounts.sysvar_instructions.key(), false),
        AccountMeta::new_readonly(ctx.accounts.recent_slothashes.key(), false),
    ];

    let mint_infos = vec![
        ctx.accounts.candy_machine.clone().to_account_info(),
        ctx.accounts.authority_pda.clone().to_account_info(),
        ctx.accounts.mint_authority.to_account_info(),
        ctx.accounts.admin.to_account_info(),
        ctx.accounts.player.clone().to_account_info(),
        ctx.accounts.asset.clone().to_account_info(),
        ctx.accounts.collection.clone().to_account_info(),
        ctx.accounts.mpl_core_program.clone().to_account_info(),
        ctx.accounts.system_program.clone().to_account_info(),
        ctx.accounts.sysvar_instructions.clone().to_account_info(),
        ctx.accounts.recent_slothashes.clone().to_account_info(),
    ];

    let ix = Instruction {
        program_id: ctx.accounts.candy_machine_program.key(),
        accounts: account_metas,
        data: MINT_DISCRIMINATOR.to_vec(),
    };

    // Invoke the Candy Machine mint instruction using the PDA signer
    invoke_signed(
        &ix,
        &mint_infos,
        &[&[
            CANDY_MACHINE_AUTHORITY_SEED.as_bytes(),
            &ctx.accounts.authority.key().to_bytes(),
            &[ctx.bumps.mint_authority],
        ]],
    )?;

    // Increment the global NFTs minted counter after a successful mint
    ctx.accounts.game_state.nfts_minted += 1;

    // Emit an event with the updated total supply (nfts_minted)
    let now: u64 = clock::Clock::get().unwrap().unix_timestamp.try_into().unwrap();
    emit!(AdminMinted {
        event_index: ctx.accounts.game_state.event_index,
        player: ctx.accounts.player.key(),
        nfts_minted: ctx.accounts.game_state.nfts_minted,
        timestamp: now,
    });

    // Update indexes
    ctx.accounts.game_state.event_index =
        ctx.accounts.game_state.event_index.checked_add(1).unwrap();

    Ok(())
}

#[event]
pub struct AdminMinted {
    pub event_index: u64,
    pub player: Pubkey,
    pub nfts_minted: u16,
    pub timestamp: u64,
}