use crate::{error::*, state::*};
use anchor_lang::prelude::*;
use anchor_lang::{
    prelude::{AccountInfo, CpiContext, Program, Result},
    system_program::{self, System, Transfer as SolanaTransfer},
    ToAccountInfo,
};
use mpl_core::accounts::BaseAssetV1;
use mpl_core::types::UpdateAuthority;
use solana_program::serialize_utils::{read_pubkey, read_u16};

pub fn calculate_trade(rt: u128, rs: u128, bs: u128) -> u128 {
    let psn_bs = PSN.checked_mul(bs).unwrap();
    let psnh_rt = PSNH.checked_mul(rt).unwrap();
    let psn_rs = PSN.checked_mul(rs).unwrap();

    let add_psn_rs_psnh_rt = psn_rs.checked_add(psnh_rt).unwrap();
    let div_add_rt = add_psn_rs_psnh_rt.checked_div(rt).unwrap();
    let add_psnd_psndh_rt = PSNH.checked_add(div_add_rt).unwrap();

    return psn_bs.checked_div(add_psnd_psndh_rt).unwrap();
}

pub fn calculate_egg_buy(amount: u128, contract_balance: u128, market_eggs: u128) -> u128 {
    let eggs: u128 = calculate_trade(amount, contract_balance, market_eggs);
    let fee: u128 = (eggs.checked_mul(FEE.into()).unwrap())
        .checked_div(100)
        .unwrap();
    return eggs.checked_sub(fee).unwrap();
}

pub fn calculate_egg_sell(eggs: u128, market_eggs: u128, contract_balance: u128) -> u128 {
    let received: u128 = calculate_trade(eggs, market_eggs, contract_balance);
    return received;
}

pub fn get_eggs_since_last_hatch(player_state: &PlayerState, game_state: &GameState) -> u128 {
    let now = Clock::get().unwrap().unix_timestamp as u128;
    let last_interaction = if player_state.last_interaction == 0 {
        game_state.premarket_end
    } else {
        player_state.last_interaction
    };
    let seconds_passed = now - last_interaction as u128;
    return seconds_passed
        .checked_mul(get_my_shrimp(player_state, game_state))
        .unwrap();
}

pub fn get_my_eggs(player_state: &PlayerState, game_state: &GameState) -> u128 {
    return player_state.extra_eggs + get_eggs_since_last_hatch(player_state, game_state);
}

pub fn get_my_shrimp(player_state: &PlayerState, game_state: &GameState) -> u128 {
    // Calculate the player's share of the premarket spending
    let player_share = if game_state.premarket_spent > 0 {
        player_state
            .premarket_spent
            .checked_mul(100000)
            .unwrap()
            .checked_div(game_state.premarket_spent)
            .unwrap()
    } else {
        0
    };

    // Calculate the amount of shrimp that would be purchased with the player's share of premarket spending
    let premarket_shrimp = if player_share > 0 {
        let shrimp_for_premarket_spent =
            calculate_egg_buy(game_state.premarket_spent as u128, 0, MARKET_START);
        shrimp_for_premarket_spent
            .checked_mul(player_share as u128)
            .unwrap()
            .checked_div(100000)
            .unwrap()
    } else {
        0
    };

    // Calculate and return the total shrimp count without modifying player_state
    player_state
        .shrimp
        .checked_add(premarket_shrimp.checked_div(EGGS_TO_HATCH_1SHRIMP).unwrap())
        .unwrap()
}

// transfer lamports from on person to another without using pda signer
pub fn transfer_lamports<'a>(
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    system_program: &Program<'a, System>,
    lamports: u64,
) -> Result<()> {
    let cpi_accounts = SolanaTransfer {
        from: from.to_account_info(),
        to: to.to_account_info(),
    };
    let cpi_program = system_program.to_account_info();

    let cpi_context = CpiContext::new(cpi_program, cpi_accounts);

    system_program::transfer(cpi_context, lamports)?;

    Ok(())
}

// transfer lamports from an owned PDA to another account
pub fn transfer_lamports_from_owned_pda<'a>(
    from: &AccountInfo<'a>,
    to: &AccountInfo<'a>,
    lamports: u64,
) -> Result<()> {
    **from.try_borrow_mut_lamports()? -= lamports;
    **to.try_borrow_mut_lamports()? += lamports;

    Ok(())
}

pub fn process_referral(
    game_state: &mut GameState,
    player_state: &mut PlayerState,
    player: Pubkey,
    referrer_state: &mut PlayerState,
    referrer: Pubkey,
    amount: u64,
    payer: Pubkey
) -> Result<(u64, u64)> {
    // Do nothing for no referrer
    if referrer == pubkey!("11111111111111111111111111111111") {
        return Ok((0, 0))
    }

    // Referrer must be a registered player
    require!(referrer_state.registered, CustomErrors::InvalidReferrer);

    // Can't be self
    require!(!referrer.eq(&player), CustomErrors::InvalidReferrer);

    // Only update the referrer for new player
    if player.eq(&payer) || 
        (player_state.current_referrer == pubkey!("11111111111111111111111111111111") && 
            player_state.premarket_spent == 0 && 
            player_state.market_spent == 0) {
            player_state.current_referrer = referrer;
    }

    // Returns (ref_fee, cashback)
    let ref_fee;
    let cashback;

    // Calculate referral fee and cashback
    ref_fee = amount
        .checked_mul(REFERRAL_FEE)
        .unwrap_or_default()
        .checked_div(100)
        .unwrap_or_default();
    cashback = amount
        .checked_mul(REFERRAL_CASHBACK)
        .unwrap_or_default()
        .checked_div(100)
        .unwrap_or_default();

    // Update referrer's total with their fee
    referrer_state.referral_total = referrer_state.referral_total.checked_add(ref_fee).unwrap();

    // Update buyer's referral total with their cashback
    player_state.referral_total = player_state.referral_total.checked_add(cashback).unwrap();

    // Add both ref fee and cashback to sell_and_ref_balance for tracking
    game_state.sell_and_ref_balance = game_state
        .sell_and_ref_balance
        .checked_add(ref_fee)
        .unwrap()
        .checked_add(cashback)
        .unwrap();

    Ok((ref_fee, cashback))
}

pub fn is_nft_holder(
    maybe_nft_asset: &Option<Account<BaseAssetV1>>,
    player: Pubkey,
    collection_key: Pubkey,
) -> Result<bool> {
    // Check that the tx included an NFT account
    if let Some(nft_asset) = maybe_nft_asset {
        // Check the owner is correct
        require!(
            nft_asset.owner.eq(&player),
            self::CustomErrors::InvalidOwner
        );
        // Verify update_authority is set to the correct collection
        if let UpdateAuthority::Collection(collection) = nft_asset.update_authority {
            // Check the collection is correct
            require!(
                collection.eq(&collection_key),
                self::CustomErrors::InvalidCollection
            );
            return Ok(true);
        } else {
            // User supplied an NFT that is not part of a collection
            return err!(self::CustomErrors::InvalidAsset);
        }
    }
    Ok(false)
}

// Allow compute budget IX and self IX only
pub static ALLOWED_PROGRAMS: &[Pubkey] = &[
    crate::ID,
    pubkey!("ComputeBudget111111111111111111111111111111"),
    pubkey!("L2TExMFKdjpN9kozasaurPirfHy9P8sbXoAN1qA3S95"), // Lighthouse (inserted to txs by Phantom)
];

pub fn limit_instructions(
    sysvar: &AccountInfo,
    max_ixs: u8,
    additional_programs: &Vec<Pubkey>,
) -> Result<()> {
    let sysvar_data = sysvar.data.borrow();

    let mut index = 0;

    let num_instructions =
        read_u16(&mut index, &sysvar_data).map_err(|_| ProgramError::InvalidAccountData)?;

    if num_instructions > max_ixs as u16 {
        msg!("Transaction had {} instructions", num_instructions);
        return err!(CustomErrors::BadInstruction);
    }

    let mut programs: Vec<Pubkey> =
        Vec::with_capacity(ALLOWED_PROGRAMS.len() + additional_programs.len());
    programs.extend(ALLOWED_PROGRAMS);
    programs.extend(additional_programs);

    'outer: for index in 0..num_instructions {
        let mut offset = 2 + (index * 2) as usize;

        // offset for the number of accounts
        offset = read_u16(&mut offset, &sysvar_data).unwrap() as usize;
        let num_accounts = read_u16(&mut offset, &sysvar_data).unwrap();

        // offset for the program id
        offset += (num_accounts as usize) * (1 + 32);
        let program_id = read_pubkey(&mut offset, &sysvar_data).unwrap();

        for program in &programs {
            if program_id == *program {
                continue 'outer;
            }
        }

        msg!("Transaction had ix with program id {}", program_id);
        return err!(CustomErrors::BadInstruction);
    }

    Ok({})
}
