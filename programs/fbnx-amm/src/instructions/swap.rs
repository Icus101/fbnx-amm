use anchor_lang::prelude::*;
//use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::{self, Mint, MintTo, TokenAccount, Transfer, Token};
use crate::state::*;
use crate::curve::{
    base::{SwapCurve, CurveType,},
    calculator::{CurveCalculator, TradeDirection},
    fees::CurveFees,
};
use crate::utils::*;
use crate::curve::{
     constant_product::ConstantProductCurve,
};
use crate::error::SwapError;

pub fn handler(ctx: Context<Swap>, amount_in: u64, minimum_amount_out: u64) -> Result<()> {
        let amm = &mut ctx.accounts.amm;
        if amm.to_account_info().owner != ctx.program_id {
            return Err(ProgramError::IncorrectProgramId.into());
        }

        if *ctx.accounts.pool_authority.key
            != authority_id(ctx.program_id, amm.to_account_info().key, amm.bump_seed)?
        {
            return Err(SwapError::InvalidProgramAddress.into());
        }
        if !(*ctx.accounts.swap_source.to_account_info().key == amm.token_a_account
            || *ctx.accounts.swap_source.to_account_info().key == amm.token_b_account)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if !(*ctx.accounts.swap_destination.to_account_info().key == amm.token_a_account
            || *ctx.accounts.swap_destination.to_account_info().key == amm.token_b_account)
        {
            return Err(SwapError::IncorrectSwapAccount.into());
        }
        if *ctx.accounts.swap_source.to_account_info().key
            == *ctx.accounts.swap_destination.to_account_info().key
        {
            return Err(SwapError::InvalidInput.into());
        }
        if ctx.accounts.swap_source.to_account_info().key == ctx.accounts.vault_source_info.to_account_info().key {
            return Err(SwapError::InvalidInput.into());
        }
        if ctx.accounts.swap_destination.to_account_info().key == ctx.accounts.vault_destination_info.to_account_info().key
        {
            return Err(SwapError::InvalidInput.into());
        }
        if *ctx.accounts.pool_mint.to_account_info().key != amm.pool_mint {
            return Err(SwapError::IncorrectPoolMint.into());
        }
        if *ctx.accounts.fee_account.to_account_info().key != amm.pool_fee_account {
            return Err(SwapError::IncorrectFeeAccount.into());
        }
        

        let trade_direction =
            if *ctx.accounts.swap_source.to_account_info().key == amm.token_a_account {
                TradeDirection::AtoB
            } else {
                TradeDirection::BtoA
            };

        let curve = build_curve(&amm.curve).unwrap();
        let fees = build_fees(&amm.fees).unwrap();

        let result = curve
            .swap(
                u128::try_from(amount_in).unwrap(),
                u128::try_from(ctx.accounts.swap_source.amount).unwrap(),
                u128::try_from(ctx.accounts.swap_destination.amount).unwrap(),
                trade_direction,
                &fees,
            )
            .ok_or(SwapError::ZeroTradingTokens)?;
        if result.destination_amount_swapped < u128::try_from(minimum_amount_out).unwrap() {
            return Err(SwapError::ExceededSlippage.into());
        }

        let (swap_token_a_amount, swap_token_b_amount) = match trade_direction {
            TradeDirection::AtoB => (
                result.new_swap_source_amount,
                result.new_swap_destination_amount,
            ),
            TradeDirection::BtoA => (
                result.new_swap_destination_amount,
                result.new_swap_source_amount,
            ),
        };

        let seeds = &[&amm.to_account_info().key.to_bytes(), &[amm.bump_seed][..]];

        token::transfer(
            ctx.accounts
                .into_transfer_to_swap_source_context()
                .with_signer(&[&seeds[..]]),
            u64::try_from(result.source_amount_swapped).unwrap(),
        )?;

        let mut pool_token_amount = curve
            .withdraw_single_token_type_exact_out(
                result.owner_fee,
                swap_token_a_amount,
                swap_token_b_amount,
                u128::try_from(ctx.accounts.pool_mint.supply).unwrap(),
                trade_direction,
                &fees,
            )
            .ok_or(SwapError::FeeCalculationFailure)?;

        if pool_token_amount > 0 {
            // Allow error to fall through
            if *ctx.accounts.host_fee_account.key != Pubkey::new_from_array([0; 32]) {
                let host = Account::<TokenAccount>::try_from(&ctx.accounts.host_fee_account)?;
                if *ctx.accounts.pool_mint.to_account_info().key != host.mint {
                    return Err(SwapError::IncorrectPoolMint.into());
                }
                let host_fee = fees
                    .host_fee(pool_token_amount)
                    .ok_or(SwapError::FeeCalculationFailure)?;
                if host_fee > 0 {
                    pool_token_amount = pool_token_amount
                        .checked_sub(host_fee)
                        .ok_or(SwapError::FeeCalculationFailure)?;
                    token::mint_to(
                        ctx.accounts
                            .into_mint_to_host_context()
                            .with_signer(&[&seeds[..]]),
                        u64::try_from(host_fee).unwrap(),
                    )?;
                }
            }
            token::mint_to(
                ctx.accounts
                    .into_mint_to_pool_context()
                    .with_signer(&[&seeds[..]]),
                u64::try_from(pool_token_amount).unwrap(),
            )?;
        }

        token::transfer(
            ctx.accounts
                .into_transfer_to_destination_context()
                .with_signer(&[&seeds[..]]),
            u64::try_from(result.destination_amount_swapped).unwrap(),
        )?;

        Ok(())
    }

#[derive(Accounts)]
pub struct Swap<'info> {
    /// CHECK: Safe
    #[account(seeds=[b"authority", amm.key().as_ref()], bump)]
    pub pool_authority: AccountInfo<'info>,
    pub amm: Box<Account<'info, Amm>>,
    /// CHECK: Safe
    // #[account(signer)]
    // pub user_transfer_authority: AccountInfo<'info>,
    /// CHECK: Safe
    #[account(
        mut,
        constraint=vault_source_info.owner == pool_authority.key(),
    )]
    pub vault_source_info: Account<'info,TokenAccount>,
    /// CHECK: Safe
    #[account(mut,
        constraint=vault_destination_info.owner == pool_authority.key(),
    )]
    pub vault_destination_info: Account<'info,TokenAccount>,
    #[account(mut,
        has_one = owner
    )]
    pub swap_source: Account<'info, TokenAccount>,
    #[account(mut,
        has_one = owner
    )]
    pub swap_destination: Account<'info, TokenAccount>,
    pub pool_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub fee_account: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub token_program: Program<'info,Token>,
    /// CHECK: Safe
    pub host_fee_account: AccountInfo<'info>,
}

impl<'info> Swap<'info> {
    fn into_transfer_to_swap_source_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_source_info.to_account_info().clone(),
            to: self.swap_source.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_to_destination_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.swap_destination.to_account_info().clone(),
            to: self.vault_destination_info.to_account_info().clone(),
            authority: self.pool_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_mint_to_host_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.pool_mint.to_account_info().clone(),
            to: self.host_fee_account.clone(),
            authority: self.pool_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_mint_to_pool_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.pool_mint.to_account_info().clone(),
            to: self.fee_account.to_account_info().clone(),
            authority: self.pool_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}
/// Build Curve object and Fee object
pub fn build_curve(curve_input: &CurveInput) -> Result<SwapCurve> {
    let curve_type = CurveType::try_from(curve_input.curve_type).unwrap();
    let culculator: Box<dyn CurveCalculator> = match curve_type {
        CurveType::ConstantProduct => Box::new(ConstantProductCurve {}),
        // CurveType::ConstantPrice => Box::new(ConstantPriceCurve {
        //     token_b_price: curve_input.curve_parameters,
        // }),
        // CurveType::Stable => Box::new(StableCurve {
        //     amp: curve_input.curve_parameters,
        // }),
        // CurveType::Offset => Box::new(OffsetCurve {
        //     token_b_offset: curve_input.curve_parameters,
        // }),
    };
    let curve = SwapCurve {
        curve_type: curve_type,
        calculator: culculator,
    };
    Ok(curve)
}

pub fn build_fees(fees_input: &FeesInput) -> Result<CurveFees> {
    let fees = CurveFees {
        trade_fee_numerator: fees_input.trade_fee_numerator,
        trade_fee_denominator: fees_input.trade_fee_denominator,
        owner_trade_fee_numerator: fees_input.owner_trade_fee_numerator,
        owner_trade_fee_denominator: fees_input.owner_trade_fee_denominator,
        owner_withdraw_fee_numerator: fees_input.owner_withdraw_fee_numerator,
        owner_withdraw_fee_denominator: fees_input.owner_withdraw_fee_denominator,
        host_fee_numerator: fees_input.host_fee_numerator,
        host_fee_denominator: fees_input.host_fee_denominator,
    };
    Ok(fees)
}