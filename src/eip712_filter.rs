use alloc::{string::String, vec::Vec};

/// EIP-712 filtering operation type
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Eip712FilterType {
    /// Activation
    Activation,
    /// Discarded filter path
    DiscardedFilterPath(String),
    /// Message info
    MessageInfo {
        display_name: String,
        filters_count: u8,
        signature: Vec<u8>,
    },
    /// Trusted name
    TrustedName {
        display_name: String,
        name_types: Vec<u8>,
        name_sources: Vec<u8>,
        signature: Vec<u8>,
    },
    /// Date/time
    DateTime {
        display_name: String,
        signature: Vec<u8>,
    },
    /// Amount-join token
    AmountJoinToken { token_index: u8, signature: Vec<u8> },
    /// Amount-join value
    AmountJoinValue {
        display_name: String,
        token_index: u8,
        signature: Vec<u8>,
    },
    /// Raw field
    RawField {
        display_name: String,
        signature: Vec<u8>,
    },
}

/// Parameters for EIP-712 filtering operations
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip712FilterParams {
    /// Filter operation type
    pub filter_type: Eip712FilterType,
    /// Whether this filter is discarded
    pub discarded: bool,
}
