use crate::instructions::*;
use crate::account::*;
use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;
pub mod state;
pub mod helpers;
pub mod account;

declare_id!("23BCUPpfPkfCu6bmPCaLgyTR8UkruWeUnEyeC5shr1mp");

#[program]
pub mod shrimp {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>, dev1: Pubkey, dev2: Pubkey, dev3: Pubkey, premarket_end: u64, cooldown: u64, test_env: bool) -> Result<()> {
        instructions::initialize(ctx, dev1, dev2, dev3, premarket_end, cooldown, test_env)
    }
    
    pub fn set_collection(ctx: Context<SetCollection>) -> Result<()> {
        instructions::set_collection(ctx)
    }

    pub fn dev_withdraw(ctx: Context<DevWithdraw>) -> Result<()> {
        instructions::dev_withdraw(ctx)
    }

    pub fn register(ctx: Context<Register>, username: String) -> Result<()> {
        instructions::register(ctx, username)
    }

    pub fn user_withdraw(ctx: Context<UserWithdraw>) -> Result<()> {
        instructions::user_withdraw(ctx)
    }

    pub fn sell_eggs(ctx: Context<SellAndHatchAccounts>) -> Result<()> {
        instructions::sell_eggs(ctx)
    }

    pub fn hatch_eggs(ctx: Context<SellAndHatchAccounts>) -> Result<()> {
        instructions::hatch_eggs(ctx)
    }

    pub fn buy_shrimp(ctx: Context<BuyAccounts>, amount: u64) -> Result<()> {
        instructions::buy_shrimp(ctx, amount)
    }

    pub fn buy_premarket(ctx: Context<BuyAccounts>, amount: u64) -> Result<()> {
        instructions::buy_premarket(ctx, amount)
    }

    pub fn set_market(ctx: Context<SetMarket>, market_eggs: u128) -> Result<()> {
        instructions::set_market(ctx, market_eggs)
    }

    pub fn mint_nft(ctx: Context<MintNft>) -> Result<()> {
        instructions::mint_nft(ctx)
    }

    pub fn set_program_guards(ctx: Context<SetProgramGuards>, max_ixs: u8, program_whitelist: Vec<Pubkey>) -> Result<()> {
        instructions::set_program_guards(ctx, max_ixs, program_whitelist)
    }

    pub fn end_premarket(ctx: Context<EndPremarket>) -> Result<()> {
        instructions::end_premarket(ctx)
    }

    pub fn testnet_bonus(ctx: Context<TestnetBonus>) -> Result<()> {
        instructions::testnet_bonus(ctx)
    }
}
