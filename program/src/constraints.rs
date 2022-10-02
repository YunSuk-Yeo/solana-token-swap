//! Various constraints as required for production environments

use crate::error::SwapError;
use crate::fees::Fees;

use solana_program::program_error::ProgramError;

/// Validate the given supply on initialization. This is useful for curves
/// that allow zero supply on one or both sides, since the standard constant
/// product curve must have a non-zero supply on both sides.
pub fn validate_supply(token_a_amount: u64, token_b_amount: u64) -> Result<(), SwapError> {
    if token_a_amount == 0 {
        return Err(SwapError::EmptySupply);
    }
    if token_b_amount == 0 {
        return Err(SwapError::EmptySupply);
    }
    Ok(())
}

/// Checks that the provided curve is valid for the given constraints
pub fn validate_fees(fees: &Fees) -> Result<(), ProgramError> {
    // fee should be smaller than 33% and non-zero
    if fees.trade_fee_denominator > fees.trade_fee_numerator * 3 {
        Ok(())
    } else {
        Err(SwapError::InvalidFee.into())
    }
}
