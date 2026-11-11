pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("GK2NixKg2GDEmQjsWtF6NfYA1ZMjR1cEcaLMGK8Uj48S");

#[program]
pub mod escrow_program {
    use super::*;

    pub fn make(ctx: Context<Make>, seed: u64, receive: u64, amount: u64) -> Result<()> {
        ctx.accounts.make(seed, receive, amount, &ctx.bumps)
    }

    pub fn take(ctx: Context<Take>) -> Result<()> {
        ctx.accounts.deposit()?;
        ctx.accounts.withdraw_and_close_vault()
    }

    pub fn refund(ctx: Context<Refund>) -> Result<()> {
        ctx.accounts.refund_and_close_vault()
    }

    pub fn update(ctx: Context<Update>, receive: u64) -> Result<()> {
        ctx.accounts.update(receive)
    }
}
