#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

mod consts;
pub mod eip712;
pub mod eip712_filter;
pub mod parser;
pub(crate) mod test_utils;
pub mod types;
pub mod utils;

pub use alloy_dyn_abi::{Eip712Domain, Eip712Types, Resolver, TypedData};
pub use consts::*;
