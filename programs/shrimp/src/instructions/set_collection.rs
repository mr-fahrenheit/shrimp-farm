use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::invoke_signed;
use solana_program::{instruction::Instruction, pubkey::Pubkey};

pub const SET_MINT_AUTHORITY_DISCRIMINATOR: [u8; 8] = [67, 127, 155, 187, 100, 174, 103, 121];

#[derive(Accounts)]
pub struct SetCollection<'info> {
    #[account(mut, address = game_state.authority)]
    authority: Signer<'info>,

    #[account(
        mut, 
        seeds = [GameState::SEED, authority.key().as_ref()], 
        bump
    )]
    pub game_state: Account<'info, GameState>,

    /// Candy machine account.
    /// CHECK: Can be any account
    #[account(mut)]
    pub candy_machine: UncheckedAccount<'info>,

    #[account(
        seeds = [CANDY_MACHINE_AUTHORITY_SEED.as_bytes(), authority.key().as_ref()], 
        bump
    )]
    pub nft_mint_authority: UncheckedAccount<'info>,

    #[account()]
    pub candy_machine_authority: Signer<'info>,

    /// Candy machine program
    /// CHECK: custom constraint
    #[account(address = pubkey!("CMACYFENjoBMHzapRXyo1JZkVS6EtaDDzkjMrmQLvr4J"))]
    pub candy_machine_program: UncheckedAccount<'info>,
}

pub fn set_collection(
    ctx: Context<SetCollection>
) -> Result<()> {
    // Store the collection and CandyMachine pubkey in the game state
    let game_state = &mut ctx.accounts.game_state;

    // Prevent setting if already set
    require!(game_state.collection_key == pubkey!("11111111111111111111111111111111"), CustomErrors::CollectionAlreadySet);

    // Deserialize and retrieve collection key from the supplied candy machine
    {
        let mut data_slice: &[u8] = &ctx.accounts.candy_machine.data.borrow();
        let candy_machine_data: CandyMachine = AccountDeserialize::try_deserialize(&mut data_slice).unwrap();
        game_state.collection_key = candy_machine_data.collection_mint;
        game_state.candymachine_key = *ctx.accounts.candy_machine.key;
    }

    // Construct the required accounts for the CPI call
    let accounts = vec![
        AccountMeta::new(*ctx.accounts.candy_machine.key, false),
        AccountMeta::new_readonly(*ctx.accounts.candy_machine_authority.key, true),
        AccountMeta::new_readonly(*ctx.accounts.nft_mint_authority.key, true),
    ];

    let mut account_infos = vec![
        ctx.accounts.candy_machine.to_account_info(),
        ctx.accounts.candy_machine_authority.to_account_info(),
        ctx.accounts.nft_mint_authority.to_account_info(),
    ];

    // Make nft_mint_authority as a signer for this CPI
    account_infos[2].is_signer = true;

    // Construct the instruction data
    let ix = Instruction {
        program_id: ctx.accounts.candy_machine_program.key(),
        accounts,
        data: SET_MINT_AUTHORITY_DISCRIMINATOR.to_vec(),
    };

    // Invoke with PDA signer
    invoke_signed(
        &ix,
        &account_infos.as_slice(),
        &[
            &[
                CANDY_MACHINE_AUTHORITY_SEED.as_bytes(),
                &ctx.accounts.authority.key().to_bytes(),
                &[ctx.bumps.nft_mint_authority],
            ]
        ],
    )?;

    // Emit event
    emit!(NFTCollectionSet {
        collection: game_state.collection_key,
        authority: ctx.accounts.nft_mint_authority.key()
    });

    Ok(())
}

#[event]
struct NFTCollectionSet {
    collection: Pubkey,
    authority: Pubkey,
}

/// Candy machine configuration data.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct CandyMachineData {
    /// Number of assets available
    pub items_available: u64,
    /// Max supply of each individual asset (default 0)
    pub max_supply: u64,
    /// Indicates if the asset is mutable or not (default yes)
    pub is_mutable: bool,
    /// Config line settings
    pub config_line_settings: Option<ConfigLineSettings>,
    /// Hidden setttings
    pub hidden_settings: Option<HiddenSettings>,
}

/// Candy machine state and config data.
#[account]
#[derive(Default, Debug)]
pub struct CandyMachine {
    /// Authority address.
    pub authority: Pubkey,
    /// Authority address allowed to mint from the candy machine.
    pub mint_authority: Pubkey,
    /// The collection mint for the candy machine.
    pub collection_mint: Pubkey,
    /// Number of assets redeemed.
    pub items_redeemed: u64,
    /// Candy machine configuration data.
    pub data: CandyMachineData,
    // hidden data section to avoid deserialisation:
    //
    // - (u32) how many actual lines of data there are currently (eventually
    //   equals items available)
    // - (ConfigLine * items_available) lines and lines of name + uri data
    // - (item_available / 8) + 1 bit mask to keep track of which ConfigLines
    //   have been added
    // - (u32 * items_available) mint indices
}

/// Hidden settings for large mints used with off-chain data.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct HiddenSettings {
    /// Asset prefix name
    pub name: String,
    /// Shared URI
    pub uri: String,
    /// Hash of the hidden settings file
    pub hash: [u8; 32],
}

/// Config line settings to allocate space for individual name + URI.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, Debug)]
pub struct ConfigLineSettings {
    /// Common name prefix
    pub prefix_name: String,
    /// Length of the remaining part of the name
    pub name_length: u32,
    /// Common URI prefix
    pub prefix_uri: String,
    /// Length of the remaining part of the URI
    pub uri_length: u32,
    /// Indicates whether to use a senquential index generator or not
    pub is_sequential: bool,
}