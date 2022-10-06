use anchor_lang::prelude::*;
//use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::{self, Mint, MintTo, TokenAccount, Transfer, Token};

use crate::utils::*;
use crate::{curve::{
    base::{SwapCurve,CurveType},
    calculator::{CurveCalculator, TradeDirection},
    fees::CurveFees,
}, state::*};

use crate::error::SwapError;

use crate::curve::{
     constant_product::ConstantProductCurve,
};

pub fn handler(
    ctx: Context<DepositSingleTokenType>,
    source_token_amount: u64,
    minimum_pool_token_amount: u64,
) -> Result<()> {
    let amm = &mut ctx.accounts.amm;

    let curve = build_curve(&amm.curve).unwrap();
    let fees = build_fees(&amm.fees).unwrap();

    let trade_direction = if ctx.accounts.source.mint == ctx.accounts.swap_token_a.mint {
        TradeDirection::AtoB
    } else if ctx.accounts.source.mint == ctx.accounts.swap_token_b.mint {
        TradeDirection::BtoA
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    let source = ctx.accounts.source.to_account_info().clone();
    let (source_a_info, source_b_info) = match trade_direction {
        TradeDirection::AtoB => (Some(&source), None),
        TradeDirection::BtoA => (None, Some(&source)),
    };

    check_accounts(
        amm,
        ctx.program_id,
        &amm.to_account_info(),
        &ctx.accounts.authority,
        &ctx.accounts.swap_token_a.to_account_info(),
        &ctx.accounts.swap_token_b.to_account_info(),
        &ctx.accounts.pool_mint.to_account_info(),
        &ctx.accounts.token_program,
        source_a_info,
        source_b_info,
        None,
    )?;

    let pool_mint_supply = u128::try_from(ctx.accounts.pool_mint.supply).unwrap();
    let pool_token_amount = if pool_mint_supply > 0 {
        curve
            .deposit_single_token_type(
                u128::try_from(source_token_amount).unwrap(),
                u128::try_from(ctx.accounts.swap_token_a.amount).unwrap(),
                u128::try_from(ctx.accounts.swap_token_b.amount).unwrap(),
                pool_mint_supply,
                trade_direction,
                &fees,
            )
            .ok_or(SwapError::ZeroTradingTokens)?
    } else {
        curve.calculator.new_pool_supply()
    };

    let seeds = &[&amm.to_account_info().key.to_bytes(), &[amm.bump_seed][..]];
    let pool_token_amount = u64::try_from(pool_token_amount).unwrap();
    if pool_token_amount < minimum_pool_token_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if pool_token_amount == 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }

    match trade_direction {
        TradeDirection::AtoB => {
            token::transfer(
                ctx.accounts.into_transfer_to_token_a_context(),
                source_token_amount,
            )?;
        }
        TradeDirection::BtoA => {
            token::transfer(
                ctx.accounts.into_transfer_to_token_b_context(),
                source_token_amount,
            )?;
        }
    }
    token::mint_to(
        ctx.accounts
            .into_mint_to_context()
            .with_signer(&[&seeds[..]]),
        pool_token_amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositSingleTokenType<'info> {
    #[account(mut)]
    pub amm: Box<Account<'info, Amm>>,
    /// CHECK: Safe
    #[account(seeds=[b"authority".as_ref(), amm.key().as_ref()], bump)]
    pub authority: AccountInfo<'info>,
    /// CHECK: Safe
    pub owner: Signer<'info>,
    #[account(mut,
        has_one = owner
    )]
    pub source: Account<'info, TokenAccount>,
    #[account(mut,
        constraint = swap_token_a.owner == authority.key(),
    )]
    pub swap_token_a: Account<'info, TokenAccount>,
    #[account(mut,
        constraint = swap_token_b.owner == authority.key(),
    )]
    pub swap_token_b: Account<'info, TokenAccount>,
    #[account(mut,
        constraint = destination.mint == pool_mint.key(),
    )]
    pub pool_mint: Account<'info, Mint>,
    /// CHECK: Safe
    #[account(mut,
        token::mint = pool_mint.key(),
    )]
    pub destination: Account<'info,TokenAccount>,
    /// CHECK: Safe
    pub token_program: Program<'info,Token>,
}

impl<'info> DepositSingleTokenType<'info> {
    fn into_transfer_to_token_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source.to_account_info().clone(),
            to: self.swap_token_a.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_to_token_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source.to_account_info().clone(),
            to: self.swap_token_b.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_mint_to_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.pool_mint.to_account_info().clone(),
            to: self.destination.to_account_info().clone(),
            authority: self.authority.clone(),
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