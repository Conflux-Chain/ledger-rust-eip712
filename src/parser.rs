use crate::types::Eip712StructDefinitions;
use crate::utils::*;

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
    borrow::ToOwned,
};
use alloy_primitives::hex;
use serde_json::{Number, Value};

pub enum TypeSchema {
    Primitive { name: String, size: Option<u8> },
    Array { item: Box<TypeSchema> },
    Struct { fields: Vec<Field> },
}

pub struct Field {
    name: String,
    ty: TypeSchema,
}

pub fn build_schema(
    struct_defs: &Eip712StructDefinitions,
    type_name: &String,
) -> Result<TypeSchema, String> {
    let field_defs = struct_defs.get(type_name).ok_or("not found")?;

    let mut fields = Vec::new();

    for fd in field_defs.iter() {
        let mut ty = if fd.is_struct() {
            let custom_type_name = fd
                .field_type
                .custom_type_name()
                .expect("should exist")
                .to_string();
            build_schema(struct_defs, &custom_type_name)?
        } else {
            let (name, size) = fd.primitive_type_string_and_size();
            TypeSchema::Primitive { name, size }
        };
        let ty = if fd.is_array() {
            for _ in 0..fd.array_levels.len() {
                ty = TypeSchema::Array { item: Box::new(ty) }
            }
            ty
        } else {
            ty
        };

        fields.push(Field {
            name: fd.name.clone(),
            ty,
        });
    }

    return Ok(TypeSchema::Struct { fields });
}

// from type schema and raw data build serde_json::Value
pub fn build_value(
    schema: &TypeSchema,
    data: &mut impl Iterator<Item = Vec<u8>>,
) -> Result<Value, String> {
    let res = match schema {
        TypeSchema::Primitive { name, size } => {
            let raw = data.next().ok_or("invalid")?;
            match name.as_str() {
                "bool" => Value::Bool(raw[0] == 1),
                "int" => {
                    if let Some(s) = size {
                        if raw.len() > *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    if raw.len() <= 16 {
                        let val = parse_i128(&raw).map_err(|err| err.to_string())?;
                        match Number::from_i128(val) {
                            Some(num) => Value::Number(num),
                            None => Value::String(format!("{:#x}", val)),
                        }
                    } else {
                        let val = parse_i256(&raw).map_err(|err| err.to_string())?;
                        Value::String(val.to_hex_string())
                    }
                }
                "uint" => {
                    if let Some(s) = size {
                        if raw.len() > *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    if raw.len() <= 16 {
                        let val = parse_u128(&raw).map_err(|err| err.to_string())?;
                        match Number::from_u128(val) {
                            Some(num) => Value::Number(num),
                            None => Value::String(format!("{:#x}", val)),
                        }
                    } else {
                        let val = parse_u256(&raw).map_err(|err| err.to_string())?;
                        let hex_str = format!("{:#x}", val);
                        Value::String(hex_str)
                    }
                }
                "bytes" => {
                    if let Some(s) = size {
                        if raw.len() != *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    let hex_str = format!("0x{}", hex::encode(&raw));
                    Value::String(hex_str)
                }
                "string" => {
                    let val = parse_utf8_string(&raw).map_err(|err| err.to_string())?;
                    Value::String(val)
                }
                "address" => {
                    if raw.len() != 20 {
                        return Err("invalid len".to_string());
                    }
                    let addr_hex_str = format!("0x{}", hex::encode(&raw));
                    Value::String(addr_hex_str)
                }
                _ => {
                    unreachable!();
                }
            }
        }
        TypeSchema::Array { item } => {
            let len_v = data.next().ok_or("invalid")?;
            if len_v.len() != 1 {
                return Err("invalid len".to_string());
            }
            let len = len_v[0];
            let mut arr = vec![];

            for _ in 0..len {
                arr.push(build_value(item, data)?);
            }

            arr.into()
        }
        TypeSchema::Struct { fields } => {
            let mut obj = serde_json::Map::new();
            for f in fields {
                let value = build_value(&f.ty, data)?;
                obj.insert(f.name.clone(), value);
            }
            Value::Object(obj)
        }
    };
    Ok(res)
}

#[derive(Debug)]
pub struct UIField {
    pub name: String,
    pub value: String,
}

pub fn build_ui_fields(
    schema: &TypeSchema,
    data: &mut impl Iterator<Item = Vec<u8>>,
    field_name: &str, // used for primitives
) -> Result<Vec<UIField>, String> {
    let res = match schema {
        TypeSchema::Primitive { name, size } => {
            let raw = data.next().ok_or("invalid")?;
            let field = match name.as_str() {
                "bool" => UIField {
                    name: field_name.to_owned(),
                    value: if raw[0] == 1 {
                        "true".to_string()
                    } else {
                        "false".to_string()
                    },
                },
                "int" => {
                    if let Some(s) = size {
                        if raw.len() > *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    let value = if raw.len() <= 16 {
                        let val = parse_i128(&raw).map_err(|err| err.to_string())?;
                        format!("{}", val)
                    } else {
                        let val = parse_i256(&raw).map_err(|err| err.to_string())?;
                        format!("{}", val)
                    };
                    UIField {
                        name: field_name.to_owned(),
                        value,
                    }
                }
                "uint" => {
                    if let Some(s) = size {
                        if raw.len() > *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    let value = if raw.len() <= 16 {
                        let val = parse_u128(&raw).map_err(|err| err.to_string())?;
                        format!("{}", val)
                    } else {
                        let val = parse_u256(&raw).map_err(|err| err.to_string())?;
                        format!("{}", val)
                    };
                    UIField {
                        name: field_name.to_owned(),
                        value,
                    }
                }
                "bytes" => {
                    if let Some(s) = size {
                        if raw.len() != *s as usize {
                            return Err("invalid len".to_string());
                        }
                    }
                    let hex_str = format!("0x{}", hex::encode(&raw));
                    UIField {
                        name: field_name.to_owned(),
                        value: hex_str,
                    }
                }
                "string" => {
                    let val = parse_utf8_string(&raw).map_err(|err| err.to_string())?;
                    UIField {
                        name: field_name.to_owned(),
                        value: val,
                    }
                }
                "address" => {
                    if raw.len() != 20 {
                        return Err("invalid len".to_string());
                    }
                    let addr_hex_str = format!("0x{}", hex::encode(&raw));
                    UIField {
                        name: field_name.to_owned(),
                        value: addr_hex_str,
                    }
                }
                _ => {
                    unreachable!();
                }
            };
            vec![field]
        }
        TypeSchema::Array { item } => {
            let len_v = data.next().ok_or("invalid")?;
            if len_v.len() != 1 {
                return Err("invalid len".to_string());
            }
            let len = len_v[0];
            let mut arr = vec![];

            for _ in 0..len {
                arr.extend(build_ui_fields(item, data, field_name)?);
            }

            arr
        }
        TypeSchema::Struct { fields } => {
            let mut arr = vec![];
            for f in fields {
                let res = build_ui_fields(&f.ty, data, &f.name)?;
                arr.extend(res);
            }
            arr
        }
    };
    Ok(res)
}

#[cfg(test)]
mod tests {
    use alloy_primitives::hex;
    // use alloy_sol_types::Eip712Domain;
    use super::{build_schema, build_value, build_ui_fields};
    use crate::types::{
        Eip712ArrayLevel, Eip712FieldDefinition, Eip712FieldType, Eip712StructDefinitions,
        build_resolver_from_struct_defs,
    };
    use alloy_dyn_abi::eip712::TypedData;

    fn get_raw_typed_data() -> Result<TypedData, String> {
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

    fn prepare_struct_defs() -> Eip712StructDefinitions {
        let mut struct_defs: Eip712StructDefinitions = Default::default();

        struct_defs.insert(
            "EIP712Domain".to_string(),
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
            ],
        );

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

    fn prepare_data() -> Vec<Vec<u8>> {
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

    #[test]
    fn test_build_value() {
        let struct_defs = prepare_struct_defs();

        let type_schema = build_schema(&struct_defs, &"Mail".to_string());
        assert_eq!(type_schema.is_ok(), true);
        let type_schema = type_schema.unwrap();

        let data = prepare_data();
        let value = build_value(&type_schema, &mut data.into_iter());
        assert_eq!(value.is_ok(), true);
        let value = value.unwrap();

        let typed = get_raw_typed_data().expect("success");

        let resolver = build_resolver_from_struct_defs(&struct_defs).unwrap();

        let new_typed_data = TypedData {
            domain: typed.domain.clone(),
            resolver,
            primary_type: "Mail".to_string(),
            message: value,
        };

        let hash1 = typed.eip712_signing_hash().unwrap();
        let maybe_hash2 = new_typed_data.eip712_signing_hash();

        assert!(maybe_hash2.is_ok());

        assert_eq!(hash1, maybe_hash2.unwrap());
    }

    #[test]
    fn test_build_ui_field() {
        let struct_defs = prepare_struct_defs();

        let type_schema = build_schema(&struct_defs, &"Mail".to_string());
        assert_eq!(type_schema.is_ok(), true);
        let type_schema = type_schema.unwrap();

        let data = prepare_data();

        let ui_fields = build_ui_fields(&type_schema, &mut data.into_iter(), "");
        assert!(ui_fields.is_ok());
        let ui_fields = ui_fields.unwrap();
        println!("{:?}", ui_fields);
        assert!(ui_fields.len() > 0);
    }
}
