use anchor_lang::prelude::*;

pub mod error; 
pub mod state; 
pub mod instructions;
pub mod curve;
//pub mod utils;

use instructions::*;
use crate::state::*;

declare_id!("FzbA7oybquXL6sNn71hPU2RQh1kbRiDtCGbwmk34VFR2");

#[program]
pub mod fbnx_amm {
    use super::*;

    pub fn init_pool(ctx: Context<Initialize>,fees_input:FeesInput,curve_input:CurveInput) -> Result<()> {
        init_pool::handler(ctx,fees_input,curve_input)?;
        Ok(())
    }

    pub fn swap(ctx: Context<Swap>,amount_in : u64,minimum_amount_out : u64) -> Result<()> {
        swap::handler(ctx,amount_in,minimum_amount_out)?;
        Ok(())
    }

    pub fn deposit_all(ctx: Context<DepositAllTokenTypes>,
        pool_token_amount: u64,
        maximum_token_a_amount: u64,
        maximum_token_b_amount: u64,) -> Result<()> {
        deposit_all::handler(ctx,pool_token_amount,maximum_token_a_amount,maximum_token_b_amount)?;
        Ok(())
    }

    pub fn deposit_single(ctx: Context<DepositSingleTokenType>,
        source_token_amount: u64,
        maximum_pool_token_amount: u64,) -> Result<()> {
        deposit_single::handler(ctx,source_token_amount,maximum_pool_token_amount)?;
        Ok(())
    }

    pub fn withdraw_single(ctx: Context<WithdrawSingleTokenType>,
        destination_token_amount: u64,
        maximum_pool_token_amount: u64,) -> Result<()> {
        withdraw_single::handler(ctx,destination_token_amount,maximum_pool_token_amount)?;
        Ok(())
    }

    pub fn withdraw_all(ctx: Context<WithdrawAllTokenTypes>,
        pool_token_amount: u64,
        minimum_token_a_amount: u64,
        minimum_token_b_amount: u64,) -> Result<()> {
        withdraw_all::handler(ctx,pool_token_amount,minimum_token_a_amount,minimum_token_b_amount)?;
        Ok(())
    }


}


