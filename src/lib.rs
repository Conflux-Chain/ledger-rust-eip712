#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod eip712_filter;
pub mod parser;
pub mod types;
pub mod utils;

pub use alloy_dyn_abi::{Eip712Domain, Eip712Types, Resolver, TypedData};
