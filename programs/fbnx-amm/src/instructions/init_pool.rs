use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, MintTo,Token, TokenAccount};


use crate:: state::*;
use crate::utils::*;
use crate::error::SwapError;

pub fn handler(
    ctx: Context<Initialize>,
    fees_input: FeesInput,
    curve_input: CurveInput,
) -> Result<()> {
    if ctx.accounts.amm.is_initialized {
        return Err(SwapError::AlreadyInUse.into());
    }

    let (_swap_authority, bump_seed) = Pubkey::find_program_address(
         &[&ctx.accounts.amm.to_account_info().key.to_bytes()],
         ctx.program_id,
     );
    let seeds = &[
         &ctx.accounts.amm.to_account_info().key.to_bytes(),
         &[bump_seed][..],
     ];

    // if *ctx.accounts.pool_authority.key != swap_authority {
    //     return Err(SwapError::InvalidProgramAddress.into());
    // }
    if *ctx.accounts.pool_authority.key != ctx.accounts.vault0.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if *ctx.accounts.pool_authority.key != ctx.accounts.vault1.owner {
        return Err(SwapError::InvalidOwner.into());
    }
    if *ctx.accounts.pool_authority.key == ctx.accounts.destination.owner {
        return Err(SwapError::InvalidOutputOwner.into());
    }
    if *ctx.accounts.pool_authority.key == ctx.accounts.fee_account.owner {
        return Err(SwapError::InvalidOutputOwner.into());
    }
    if COption::Some(*ctx.accounts.pool_authority.key) != ctx.accounts.pool_mint.mint_authority {
        return Err(SwapError::InvalidOwner.into());
    }

    if ctx.accounts.vault0.mint == ctx.accounts.vault1.mint {
        return Err(SwapError::RepeatedMint.into());
    }

    let curve = build_curve(&curve_input).unwrap();
    curve
        .calculator
        .validate_supply(ctx.accounts.vault0.amount, ctx.accounts.vault1.amount)?;
    if ctx.accounts.vault0.delegate.is_some() {
        return Err(SwapError::InvalidDelegate.into());
    }
    if ctx.accounts.vault1.delegate.is_some() {
        return Err(SwapError::InvalidDelegate.into());
    }
    if ctx.accounts.vault0.close_authority.is_some() {
        return Err(SwapError::InvalidCloseAuthority.into());
    }
    if ctx.accounts.vault1.close_authority.is_some() {
        return Err(SwapError::InvalidCloseAuthority.into());
    }

    if ctx.accounts.pool_mint.supply != 0 {
        return Err(SwapError::InvalidSupply.into());
    }
    if ctx.accounts.pool_mint.freeze_authority.is_some() {
        return Err(SwapError::InvalidFreezeAuthority.into());
    }
    if *ctx.accounts.pool_mint.to_account_info().key != ctx.accounts.fee_account.mint {
        return Err(SwapError::IncorrectPoolMint.into());
    }
    let fees = build_fees(&fees_input).unwrap();

    if let Some(swap_constraints) = SWAP_CONSTRAINTS {
        let owner_key = swap_constraints
            .owner_key
            .parse::<Pubkey>()
            .map_err(|_| SwapError::InvalidOwner)?;
        if ctx.accounts.fee_account.owner != owner_key {
            return Err(SwapError::InvalidOwner.into());
        }
        swap_constraints.validate_curve(&curve)?;
        swap_constraints.validate_fees(&fees)?;
    }
    fees.validate()?;
    curve.calculator.validate()?;

    let initial_amount = curve.calculator.new_pool_supply();

    token::mint_to(
        ctx.accounts
            .into_mint_to_context()
            .with_signer(&[&seeds[..]]),
        u64::try_from(initial_amount).unwrap(),
    )?;

    let amm = &mut ctx.accounts.amm;
    amm.is_initialized = true;
    amm.bump_seed = bump_seed;
    amm.token_program_id = *ctx.accounts.token_program.key;
    amm.token_a_account = *ctx.accounts.vault0.to_account_info().key;
    amm.token_b_account = *ctx.accounts.vault1.to_account_info().key;
    amm.pool_mint = *ctx.accounts.pool_mint.to_account_info().key;
    amm.token_a_mint = ctx.accounts.vault0.mint;
    amm.token_b_mint = ctx.accounts.vault1.mint;
    amm.pool_fee_account = *ctx.accounts.fee_account.to_account_info().key;
    amm.fees = fees_input;
    amm.curve = curve_input;

    Ok(())
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// CHECK: Safe
    #[account(seeds=[b"authority", amm.key().as_ref()], bump)]
    pub pool_authority: AccountInfo<'info>,
    #[account(
         zero         
    )]
    pub amm: Box<Account<'info, Amm>>,
    // pool mint : used to track relative contribution amount of LPs
    #[account(
        init, 
        payer=payer,
        seeds=[b"pool_mint", amm.key().as_ref()], 
        bump, 
        mint::decimals = 2,
        mint::authority = pool_authority
    )] 
    pub pool_mint: Account<'info, Mint>,
    // account to hold token X
    #[account(
        init, 
        payer=payer, 
        seeds=[b"vault0", amm.key().as_ref()], 
        bump,
        token::mint = mint0,
        token::authority = pool_authority
    )]
    pub vault0: Box<Account<'info, TokenAccount>>,
    #[account(
        init, 
        payer=payer, 
        seeds=[b"vault1", amm.key().as_ref()],
        bump,
        token::mint = mint1,
        token::authority = pool_authority
    )]
    pub vault1: Box<Account<'info, TokenAccount>>, 
    #[account(mut)]
    pub fee_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub destination: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    // pool for token_x -> token_y 
    pub mint0: Account<'info, Mint>,
    pub mint1: Account<'info, Mint>,
    pub token_program: Program<'info,Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info,System>
}

impl<'info> Initialize<'info> {
    fn into_mint_to_context(&self) -> CpiContext<'_, '_, '_, 'info, MintTo<'info>> {
        let cpi_accounts = MintTo {
            mint: self.pool_mint.to_account_info().clone(),
            to: self.destination.to_account_info().clone(),
            authority: self.pool_authority.clone(),
        };
        CpiContext::new(self.token_program.to_account_info().clone(), cpi_accounts)
    }
}
