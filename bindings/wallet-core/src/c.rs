//! This module expose handy C compatible functions to reuse in the different
//! C style bindings that we have (wallet-c, wallet-jni...)

use crate::{Conversion, Error, Proposal, Result, VotePlan, Wallet};
use chain_impl_mockchain::{
    certificate::VotePlanId,
    transaction::Input,
    value::Value,
    vote::{Choice, Options as VoteOptions},
};
use std::convert::{TryFrom, TryInto};
use thiserror::Error;
pub use wallet::Settings;

pub type WalletPtr = *mut Wallet;
pub type SettingsPtr = *mut Settings;
pub type ConversionPtr = *mut Conversion;
pub type VotePlanPtr = *mut VotePlan;
pub type ProposalPtr = *mut Proposal;
pub type ErrorPtr = *mut Error;

#[derive(Debug, Error)]
#[error("null pointer")]
struct NulPtr;

#[derive(Debug, Error)]
#[error("access out of bound")]
struct OutOfBound;

/// retrieve a wallet from the given mnemonics, password and protocol magic
///
/// this function will work for all yoroi, daedalus and other wallets
/// as it will try every kind of wallet anyway
///
/// You can also use this function to recover a wallet even after you have
/// transferred all the funds to the new format (see the _convert_ function)
///
/// The recovered wallet will be returned in `wallet_out`.
///
/// # parameters
///
/// * mnemonics: a null terminated utf8 string (already normalized NFKD) in english;
/// * password: pointer to the password (in bytes, can be UTF8 string or a bytes of anything);
///   this value is optional and passing a null pointer will result in no password;
/// * password_length: the length of the password;
/// * wallet_out: a pointer to a pointer. The recovered wallet will be allocated on this pointer;
///
/// # Safety
///
/// This function dereference raw pointers (password and wallet_out). Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
/// # errors
///
/// The function may fail if:
///
/// * the mnemonics are not valid (invalid length or checksum);
/// * the `wallet_out` is null pointer
///
pub unsafe fn wallet_recover(
    mnemonics: &str,
    password: *const u8,
    password_length: usize,
    wallet_out: *mut WalletPtr,
) -> Result {
    let wallet_out: &mut WalletPtr = if let Some(wallet_out) = wallet_out.as_mut() {
        wallet_out
    } else {
        return Error::invalid_input("wallet_out").with(NulPtr).into();
    };

    let result = if !password.is_null() && password_length > 0 {
        todo!()
    } else {
        Wallet::recover(mnemonics, &[])
    };

    match result {
        Ok(wallet) => {
            *wallet_out = Box::into_raw(Box::new(wallet));
            Result::success()
        }
        Err(err) => err.into(),
    }
}

/// get the wallet id
///
/// This ID is the identifier to use against the blockchain/explorer to retrieve
/// the state of the wallet (counter, total value etc...)
///
/// # Parameters
///
/// * wallet: the recovered wallet (see recover function);
/// * id_out: a ready allocated pointer to an array of 32bytes. If this array is not
///   32bytes this may result in a buffer overflow.
///
/// # Safety
///
/// This function dereference raw pointers (wallet and id_out). Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
/// the `id_out` needs to be ready allocated 32bytes memory. If not this will result
/// in an undefined behavior, in the best scenario it will be a buffer overflow.
///
/// # Errors
///
/// * this function may fail if the wallet pointer is null;
///
pub unsafe fn wallet_id(wallet: WalletPtr, id_out: *mut u8) -> Result {
    let wallet: &Wallet = if let Some(wallet) = wallet.as_ref() {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };
    if id_out.is_null() {
        return Error::invalid_input("id_out").with(NulPtr).into();
    }

    let id = wallet.id();

    let id_out = std::slice::from_raw_parts_mut(id_out, wallet::AccountId::SIZE);

    id_out.copy_from_slice(id.as_ref());

    Result::success()
}

/// retrieve funds from daedalus or yoroi wallet in the given block0 (or
/// any other blocks).
///
/// Execute this function then you can check who much funds you have
/// retrieved from the given block.
///
/// this function may take sometimes so it is better to only call this
/// function if needed.
///
/// # Safety
///
/// This function dereference raw pointers (wallet, block0 and settings_out). Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
/// # Parameters
///
/// * wallet: the recovered wallet (see recover function);
/// * block0: the pointer to the bytes of the block0;
/// * block0_length: the length of the block0 byte string;
/// * settings_out: the settings that will be parsed from the given
///   block0;
///
/// # Errors
///
/// * this function may fail if the wallet pointer is null;
/// * the block is not valid (cannot be decoded)
///
pub unsafe fn wallet_retrieve_funds(
    wallet: WalletPtr,
    block0: *const u8,
    block0_length: usize,
    settings_out: *mut SettingsPtr,
) -> Result {
    let wallet: &mut Wallet = if let Some(wallet) = wallet.as_mut() {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };
    if block0.is_null() {
        return Error::invalid_input("block0").with(NulPtr).into();
    }
    let settings_out: &mut SettingsPtr = if let Some(settings_out) = settings_out.as_mut() {
        settings_out
    } else {
        return Error::invalid_input("settings_out").with(NulPtr).into();
    };

    let block0_bytes = std::slice::from_raw_parts(block0, block0_length);

    match wallet.retrieve_funds(block0_bytes) {
        Ok(settings) => {
            *settings_out = Box::into_raw(Box::new(settings));
            Result::success()
        }
        Err(err) => err.into(),
    }
}

/// once funds have been retrieved with `iohk_jormungandr_wallet_retrieve_funds`
/// it is possible to convert all existing funds to the new wallet.
///
/// The returned arrays are transactions to send to the network in order to do the
/// funds conversion.
///
/// Don't forget to call `iohk_jormungandr_wallet_delete_conversion` to
/// properly free the memory
///
/// # Safety
///
/// This function dereference raw pointers (wallet, settings and conversion_out). Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
pub unsafe fn wallet_convert(
    wallet: WalletPtr,
    settings: SettingsPtr,
    conversion_out: *mut ConversionPtr,
) -> Result {
    let wallet: &mut Wallet = if let Some(wallet) = wallet.as_mut() {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };
    let settings = if let Some(settings) = settings.as_ref() {
        settings.clone()
    } else {
        return Error::invalid_input("settings").with(NulPtr).into();
    };
    let conversion_out: &mut ConversionPtr = if let Some(conversion_out) = conversion_out.as_mut() {
        conversion_out
    } else {
        return Error::invalid_input("conversion_out").with(NulPtr).into();
    };

    let conversion = wallet.convert(settings);

    *conversion_out = Box::into_raw(Box::new(conversion));

    Result::success()
}

/// get the number of transactions built to convert the retrieved wallet
///
/// # Safety
///
/// This function dereference raw pointers. Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
pub unsafe fn wallet_convert_transactions_size(conversion: ConversionPtr) -> usize {
    conversion
        .as_ref()
        .map(|c| c.transactions.len())
        .unwrap_or_default()
}

/// retrieve the index-nth transactions in the conversions starting from 0
/// and finishing at `size-1` where size is retrieved from
/// `iohk_jormungandr_wallet_convert_transactions_size`.
///
/// the memory allocated returned is not owned and should not be kept
/// for longer than potential call to `iohk_jormungandr_wallet_delete_conversion`
///
/// # Safety
///
/// This function dereference raw pointers. Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
pub unsafe fn wallet_convert_transactions_get(
    conversion: ConversionPtr,
    index: usize,
    transaction_out: *mut *const u8,
    transaction_size: *mut usize,
) -> Result {
    let conversion = if let Some(conversion) = conversion.as_ref() {
        conversion
    } else {
        return Error::invalid_input("conversion").with(NulPtr).into();
    };
    let transaction_out = if let Some(t) = transaction_out.as_mut() {
        t
    } else {
        return Error::invalid_input("transaction_out").with(NulPtr).into();
    };
    let transaction_size = if let Some(t) = transaction_size.as_mut() {
        t
    } else {
        return Error::invalid_input("transaction_size").with(NulPtr).into();
    };

    if let Some(t) = conversion.transactions.get(index) {
        *transaction_out = t.as_ptr();
        *transaction_size = t.len();
        Result::success()
    } else {
        Error::wallet_conversion().with(OutOfBound).into()
    }
}

/// get the total value ignored in the conversion
///
/// value_out: will returns the total value lost into dust inputs
/// ignored_out: will returns the number of dust utxos
///
/// these returned values are informational only and this show that
/// there are UTxOs entries that are unusable because of the way they
/// are populated with dusts.
///
/// # Safety
///
/// This function dereference raw pointers. Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
pub unsafe fn wallet_convert_ignored(
    conversion: ConversionPtr,
    value_out: *mut u64,
    ignored_out: *mut usize,
) -> Result {
    if let Some(c) = conversion.as_ref() {
        let v = *c
            .ignored
            .iter()
            .map(|i: &Input| i.value())
            .sum::<Value>()
            .as_ref();
        let l = c.ignored.len();

        if let Some(value_out) = value_out.as_mut() {
            *value_out = v
        }
        if let Some(ignored_out) = ignored_out.as_mut() {
            *ignored_out = l
        };

        Result::success()
    } else {
        Error::invalid_input("conversion").with(NulPtr).into()
    }
}

/// get the total value in the wallet
///
/// make sure to call `retrieve_funds` prior to calling this function
/// otherwise you will always have `0`
///
/// After calling this function the results is returned in the `total_out`.
///
/// # Errors
///
/// * this function may fail if the wallet pointer is null;
///
/// If the `total_out` pointer is null, this function does nothing
///
/// # Safety
///
/// This function dereference raw pointers. Even though
/// the function checks if the pointers are null. Mind not to put random values
/// in or you may see unexpected behaviors
///
pub unsafe fn wallet_total_value(wallet: WalletPtr, total_out: *mut u64) -> Result {
    let wallet = if let Some(wallet) = wallet.as_ref() {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };

    if let Some(total_out) = total_out.as_mut() {
        let total = wallet.total_value();

        *total_out = *total.as_ref();
    }

    Result::success()
}

/// update the wallet account state
///
/// this is the value retrieved from any jormungandr endpoint that allows to query
/// for the account state. It gives the value associated to the account as well as
/// the counter.
///
/// It is important to be sure to have an updated wallet state before doing any
/// transactions otherwise future transactions may fail to be accepted by any
/// nodes of the blockchain because of invalid signature state.
///
/// # Errors
///
/// * this function may fail if the wallet pointer is null;
///
pub fn wallet_set_state(wallet: WalletPtr, value: u64, counter: u32) -> Result {
    let wallet = if let Some(wallet) = unsafe { wallet.as_mut() } {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };
    let value = Value(value);

    wallet.set_state(value, counter);

    Result::success()
}

/// build the vote plan object
///
/// # Errors
///
/// This function may fail if:
///
/// * `id` or `vote_plan_out` is null.
/// * `payload_type` is not a valid value.
///
/// # Safety
///
/// This function dereference raw pointers. Even though the function checks if
/// the pointers are null. Mind not to put random values in or you may see
/// unexpected behaviors.
pub unsafe fn wallet_vote_plan(
    id: *const u8,
    payload_type: u8,
    vote_plan_out: *mut VotePlanPtr,
) -> Result {
    if id.is_null() {
        return Error::invalid_input("id").with(NulPtr).into();
    }

    if vote_plan_out.is_null() {
        return Error::invalid_input("vote_plan_out").with(NulPtr).into();
    }

    let payload_type = match payload_type.try_into() {
        Ok(payload_type) => payload_type,
        Err(err) => return Error::invalid_input("payload_type").with(err).into(),
    };

    let id = std::slice::from_raw_parts(id, crate::vote::VOTE_PLAN_ID_LENGTH);
    let id = match VotePlanId::try_from(id) {
        Ok(id) => id,
        Err(err) => return Error::invalid_input("id").with(err).into(),
    };

    *vote_plan_out = Box::into_raw(Box::new(VotePlan::new(id, payload_type)));

    Result::success()
}

/// build the proposal object
///
/// # Errors
///
/// This function may fail if:
///
/// * `proposal_out` is null.
/// * `num_choices` is out of the allowed range.
///
/// # Safety
///
/// This function dereference raw pointers. Even though the function checks if
/// the pointers are null. Mind not to put random values in or you may see
/// unexpected behaviors.
pub unsafe fn wallet_vote_proposal(
    index: u8,
    num_choices: u8,
    proposal_out: *mut ProposalPtr,
) -> Result {
    if proposal_out.is_null() {
        return Error::invalid_input("proposal_out").with(NulPtr).into();
    }

    let options = match VoteOptions::new_length(num_choices) {
        Ok(options) => options,
        Err(err) => return Error::invalid_input("num_choices").with(err).into(),
    };

    *proposal_out = Box::into_raw(Box::new(Proposal::new(index, options)));

    Result::success()
}

/// build the vote cast transaction
///
/// # Errors
///
/// This function may fail upon receiving a null pointer or a `choice` value
/// that does not fall within the range specified in `proposal`.
///
/// # Safety
///
/// This function dereference raw pointers. Even though the function checks if
/// the pointers are null. Mind not to put random values in or you may see
/// unexpected behaviors.
pub unsafe fn wallet_vote_cast(
    wallet: WalletPtr,
    settings: SettingsPtr,
    vote_plan: VotePlanPtr,
    proposal: ProposalPtr,
    choice: u8,
    transaction_out: *mut *const u8,
    len_out: *mut usize,
) -> Result {
    let wallet = if let Some(wallet) = wallet.as_mut() {
        wallet
    } else {
        return Error::invalid_input("wallet").with(NulPtr).into();
    };

    let settings = if let Some(settings) = settings.as_ref() {
        settings.clone()
    } else {
        return Error::invalid_input("settings").with(NulPtr).into();
    };

    let vote_plan = if let Some(vote_plan) = vote_plan.as_ref() {
        vote_plan
    } else {
        return Error::invalid_input("vote_plan").with(NulPtr).into();
    };

    let proposal = if let Some(proposal) = proposal.as_ref() {
        proposal
    } else {
        return Error::invalid_input("proposal").with(NulPtr).into();
    };

    if transaction_out.is_null() {
        return Error::invalid_input("transaction_out").with(NulPtr).into();
    }
    if len_out.is_null() {
        return Error::invalid_input("len_out").with(NulPtr).into();
    }

    let choice = Choice::new(choice);

    let transaction = match wallet.vote(settings, vote_plan, proposal, choice) {
        Ok(transaction) => transaction,
        Err(err) => return err.into(),
    };

    *transaction_out = transaction.as_ptr();
    *len_out = transaction.len();

    Result::success()
}

/// delete the pointer and free the allocated memory
pub fn wallet_delete_error(error: ErrorPtr) {
    if !error.is_null() {
        let boxed = unsafe { Box::from_raw(error) };

        std::mem::drop(boxed);
    }
}

/// delete the pointer and free the allocated memory
pub fn wallet_delete_settings(settings: SettingsPtr) {
    if !settings.is_null() {
        let boxed = unsafe { Box::from_raw(settings) };

        std::mem::drop(boxed);
    }
}

/// delete the pointer, zero all the keys and free the allocated memory
pub fn wallet_delete_wallet(wallet: WalletPtr) {
    if !wallet.is_null() {
        let boxed = unsafe { Box::from_raw(wallet) };

        std::mem::drop(boxed);
    }
}

/// delete the pointer
pub fn wallet_delete_conversion(conversion: ConversionPtr) {
    if !conversion.is_null() {
        let boxed = unsafe { Box::from_raw(conversion) };

        std::mem::drop(boxed);
    }
}

/// delete the pointer
pub fn wallet_delete_vote_plan(vote_plan: VotePlanPtr) {
    if !vote_plan.is_null() {
        let boxed = unsafe { Box::from_raw(vote_plan) };

        std::mem::drop(boxed);
    }
}

/// delete the pointer
pub fn wallet_delete_proposal(proposal: ProposalPtr) {
    if !proposal.is_null() {
        let boxed = unsafe { Box::from_raw(proposal) };

        std::mem::drop(boxed);
    }
}
