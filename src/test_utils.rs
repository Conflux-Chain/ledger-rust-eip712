#![allow(unused)]

pub use crate::types::{
    Eip712ArrayLevel, Eip712FieldDefinition, Eip712FieldType, Eip712StructDefinitions,
};
use alloc::{
    borrow::ToOwned,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use alloy_dyn_abi::eip712::TypedData;
use alloy_primitives::hex;

pub fn get_domain_struct_def() -> Vec<Eip712FieldDefinition> {
    vec![
        Eip712FieldDefinition {
            name: "name".to_string(),
            field_type: Eip712FieldType::String,
            array_levels: vec![],
        },
        Eip712FieldDefinition {
            name: "version".to_string(),
            field_type: Eip712FieldType::String,
            array_levels: vec![],
        },
        Eip712FieldDefinition {
            name: "chainId".to_string(),
            field_type: Eip712FieldType::Uint(32),
            array_levels: vec![],
        },
        Eip712FieldDefinition {
            name: "verifyingContract".to_string(),
            field_type: Eip712FieldType::Address,
            array_levels: vec![],
        },
    ]
}

pub fn prepare_mail_struct_defs() -> Eip712StructDefinitions {
    let mut struct_defs: Eip712StructDefinitions = Default::default();

    struct_defs.insert("EIP712Domain".to_string(), get_domain_struct_def());

    struct_defs.insert(
        "Mail".to_string(),
        vec![
            Eip712FieldDefinition {
                name: "from".to_string(),
                field_type: Eip712FieldType::Custom("Person".to_string()),
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "to".to_string(),
                field_type: Eip712FieldType::Custom("Person".to_string()),
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "contents".to_string(),
                field_type: Eip712FieldType::String,
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "timestamp".to_string(),
                field_type: Eip712FieldType::Uint(8),
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "amount".to_string(),
                field_type: Eip712FieldType::Uint(32),
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "payback".to_string(),
                field_type: Eip712FieldType::Uint(32),
                array_levels: vec![],
            },
        ],
    );

    struct_defs.insert(
        "Person".to_string(),
        vec![
            Eip712FieldDefinition {
                name: "name".to_string(),
                field_type: Eip712FieldType::String,
                array_levels: vec![],
            },
            Eip712FieldDefinition {
                name: "wallets".to_string(),
                field_type: Eip712FieldType::Address,
                array_levels: vec![Eip712ArrayLevel::Dynamic],
            },
        ],
    );

    struct_defs
}

pub fn get_raw_mail_typed_data() -> Result<TypedData, String> {
    let json = r#"
            {
                "domain": {
                    "chainId": 1,
                    "name": "Simple Mail",
                    "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC",
                    "version": "1"
                },
                "message": {
                    "from": {
                        "name": "Cow",
                        "wallets": [
                            "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826",
                            "0xDeaDbeefdEAdbeefdEadbEEFdeadbeEFdEaDbeeF"
                        ]
                    },
                    "to": {
                        "name": "Bob",
                        "wallets": [
                            "0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB",
                            "0xB0BdaBea57B0BDABeA57b0bdABEA57b0BDabEa57",
                            "0xB0B0b0b0b0b0B000000000000000000000000000"
                        ]
                    },
                    "contents": "Hello, Bob!",
                    "timestamp": 1633072800,
                    "amount": 1000000,
                    "payback": "0x1000000000000000000"
                },
                "primaryType": "Mail",
                "types": {
                    "EIP712Domain": [
                        { "name": "name", "type": "string" },
                        { "name": "version", "type": "string" },
                        { "name": "chainId", "type": "uint256" },
                        { "name": "verifyingContract", "type": "address" }
                    ],
                    "Mail": [
                        { "name": "from", "type": "Person" },
                        { "name": "to", "type": "Person" },
                        { "name": "contents", "type": "string" },
                        { "name": "timestamp", "type": "uint64" },
                        { "name": "amount", "type": "uint256" },
                        { "name": "payback", "type": "uint256" }
                    ],
                    "Person": [
                        { "name": "name", "type": "string" },
                        { "name": "wallets", "type": "address[]" }
                    ]
                }
            }
            "#;

    let typed: TypedData = serde_json::from_str(json).map_err(|_| "invalid json str")?;
    // let hash: B256 = typed.eip712_signing_hash().map_err(|_| "build 712 signing hash failed")?;
    Ok(typed)
}

pub fn prepare_mail_data() -> Vec<Vec<u8>> {
    vec![
        hex::decode("436f77").unwrap(),
        hex::decode("02").unwrap(),
        hex::decode("cd2a3d9f938e13cd947ec05abc7fe734df8dd826").unwrap(),
        hex::decode("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef").unwrap(),
        hex::decode("426f62").unwrap(),
        hex::decode("03").unwrap(),
        hex::decode("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
        hex::decode("b0bdabea57b0bdabea57b0bdabea57b0bdabea57").unwrap(),
        hex::decode("b0b0b0b0b0b0b000000000000000000000000000").unwrap(),
        hex::decode("48656c6c6f2c20426f6221").unwrap(),
        hex::decode("6156b6a0").unwrap(),
        hex::decode("0f4240").unwrap(),
        hex::decode("01000000000000000000").unwrap(),
    ]
}
