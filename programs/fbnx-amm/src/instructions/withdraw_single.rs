use anchor_lang::prelude::*;
//use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::{self, Burn, Mint, TokenAccount, Transfer, Token};

use crate::curve::{
    base::{SwapCurve,CurveType},
    calculator::{CurveCalculator,  TradeDirection},
    fees::CurveFees,
};
use crate::state::*;
use crate::curve::{
     constant_product::ConstantProductCurve,
};
use crate::error::SwapError;
use crate::utils::*;
pub fn handler(
    ctx: Context<WithdrawSingleTokenType>,
    destination_token_amount: u64,
    maximum_pool_token_amount: u64,
) -> Result<()> {
    let amm = &mut ctx.accounts.amm;

    let curve = build_curve(&amm.curve).unwrap();
    let fees = build_fees(&amm.fees).unwrap();

    let trade_direction = if ctx.accounts.destination.mint == ctx.accounts.swap_token_a.mint {
        TradeDirection::AtoB
    } else if ctx.accounts.destination.mint == ctx.accounts.swap_token_b.mint {
        TradeDirection::BtoA
    } else {
        return Err(SwapError::IncorrectSwapAccount.into());
    };

    let destination = ctx.accounts.destination.to_account_info().clone();
    let (destination_a_info, destination_b_info) = match trade_direction {
        TradeDirection::AtoB => (Some(&destination), None),
        TradeDirection::BtoA => (None, Some(&destination)),
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
        destination_a_info,
        destination_b_info,
        Some(&ctx.accounts.fee_account.to_account_info()),
    )?;

    let pool_mint_supply = u128::try_from(ctx.accounts.pool_mint.supply).unwrap();
    let swap_token_a_amount = u128::try_from(ctx.accounts.swap_token_a.amount).unwrap();
    let swap_token_b_amount = u128::try_from(ctx.accounts.swap_token_b.amount).unwrap();

    let burn_pool_token_amount = curve
        .withdraw_single_token_type_exact_out(
            u128::try_from(destination_token_amount).unwrap(),
            swap_token_a_amount,
            swap_token_b_amount,
            pool_mint_supply,
            trade_direction,
            &fees,
        )
        .ok_or(SwapError::ZeroTradingTokens)?;

    let withdraw_fee: u128 =
        if ctx.accounts.fee_account.key == ctx.accounts.source.to_account_info().key {
            // withdrawing from the fee account, don't assess withdraw fee
            0
        } else {
            fees.owner_withdraw_fee(burn_pool_token_amount)
                .ok_or(SwapError::FeeCalculationFailure)?
        };
    let pool_token_amount = burn_pool_token_amount
        .checked_add(withdraw_fee)
        .ok_or(SwapError::CalculationFailure)?;

    if u64::try_from(pool_token_amount).unwrap() > maximum_pool_token_amount {
        return Err(SwapError::ExceededSlippage.into());
    }
    if pool_token_amount == 0 {
        return Err(SwapError::ZeroTradingTokens.into());
    }

    let seeds = &[&amm.to_account_info().key.to_bytes(), &[amm.bump_seed][..]];

    if withdraw_fee > 0 {
        token::transfer(
            ctx.accounts.into_transfer_to_fee_account_context(),
            u64::try_from(withdraw_fee).unwrap(),
        )?;
    }
    token::burn(
        ctx.accounts.into_burn_context(),
        u64::try_from(burn_pool_token_amount).unwrap(),
    )?;

    match trade_direction {
        TradeDirection::AtoB => {
            token::transfer(
                ctx.accounts
                    .into_transfer_from_token_a_context()
                    .with_signer(&[&seeds[..]]),
                destination_token_amount,
            )?;
        }
        TradeDirection::BtoA => {
            token::transfer(
                ctx.accounts
                    .into_transfer_from_token_b_context()
                    .with_signer(&[&seeds[..]]),
                destination_token_amount,
            )?;
        }
    }

    Ok(())
}


#[derive(Accounts)]
pub struct WithdrawSingleTokenType<'info> {
    #[account(mut)]
    pub amm: Box<Account<'info, Amm>>,
    /// CHECK: Safe
    #[account(seeds=[b"authority".as_ref(), amm.key().as_ref()], bump)]
    pub authority: AccountInfo<'info>,
    /// CHECK: Safe
    pub owner: Signer<'info>,
    #[account(mut,
        token::mint = pool_mint.key()
    )]
    pub source: Account<'info, TokenAccount>,
    #[account(mut,
        constraint=swap_token_a.owner == authority.key(),
    )]
    pub swap_token_a: Account<'info, TokenAccount>,
    #[account(mut,
        constraint=swap_token_b.owner == authority.key(),
    )]
    pub swap_token_b: Account<'info, TokenAccount>,
    #[account(mut,
        mint::authority = authority
    )]
    pub pool_mint: Account<'info, Mint>,
    #[account(mut,
        has_one = owner
    )]
    pub destination: Account<'info, TokenAccount>,
    /// CHECK: Safe
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,
    /// CHECK: Safe
    pub token_program: Program<'info,Token>,
}

impl<'info> WithdrawSingleTokenType<'info> {
    fn into_transfer_to_fee_account_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.source.to_account_info().clone(),
            to: self.fee_account.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_burn_context(&self) -> CpiContext<'_, '_, '_, 'info, Burn<'info>> {
        let cpi_accounts = Burn {
            mint: self.pool_mint.to_account_info().clone(),
            from: self.source.to_account_info().clone(),
            authority: self.owner.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_from_token_a_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.swap_token_a.to_account_info().clone(),
            to: self.destination.to_account_info().clone(),
            authority: self.authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }

    fn into_transfer_from_token_b_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.swap_token_b.to_account_info().clone(),
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