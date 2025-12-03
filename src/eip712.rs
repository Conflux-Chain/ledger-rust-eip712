use crate::{
    parser::{TypeSchema, build_schema},
    types::Eip712StructDefinitions,
    utils::*,
};
use alloc::{
    borrow::ToOwned,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use alloy_primitives::{Address, B256, Bytes, utils::keccak256};
use alloy_sol_types::{Eip712Domain, SolValue};

pub fn encode_types_without_sub_type(
    struct_defs: &Eip712StructDefinitions,
) -> Result<BTreeMap<String, String>, String> {
    let mut res: BTreeMap<String, String> = Default::default();

    for (struct_name, field_defs) in struct_defs {
        let mut type_str: String = String::new();
        type_str.push_str(&format!("{}(", struct_name));
        for (index, field_def) in field_defs.iter().enumerate() {
            if index > 0 {
                type_str.push_str(",");
            }
            let field_type_str = field_def.type_string();
            type_str.push_str(&format!("{} {}", field_type_str, field_def.name));
        }
        type_str.push_str(")");

        res.insert(struct_name.to_owned(), type_str);
    }

    Ok(res)
}

// return sorted sub custom types
pub fn find_sub_custom_types(
    struct_defs: &Eip712StructDefinitions,
    type_name: &String,
) -> Result<Vec<String>, String> {
    let mut res = vec![];

    let field_defs = struct_defs
        .get(type_name)
        .ok_or(format!("{} field defs not found", type_name))?;
    for f in field_defs {
        if !f.is_struct() {
            continue;
        }

        let custom_type = f.field_type.type_string();

        let sub_custom_types = find_sub_custom_types(struct_defs, &custom_type)?;
        res.extend(sub_custom_types);

        res.push(custom_type);
    }

    // sort and removes consecutive repeated elements
    res.sort();
    res.dedup();

    Ok(res)
}

pub fn encode_type(
    struct_types: &BTreeMap<String, String>,
    struct_defs: &Eip712StructDefinitions,
    type_name: &String,
) -> Result<String, String> {
    let mut type_str = struct_types.get(type_name).ok_or("not found")?.to_owned();

    let sub_customs = find_sub_custom_types(struct_defs, type_name)?;

    for custom in &sub_customs {
        let custom_type = struct_types.get(custom).ok_or("not found")?;
        type_str.push_str(&custom_type);
    }

    Ok(type_str)
}

pub fn encode_all_struct_type(
    struct_defs: &Eip712StructDefinitions,
) -> Result<BTreeMap<String, String>, String> {
    let struct_types = encode_types_without_sub_type(struct_defs)?;
    let mut res: BTreeMap<String, String> = Default::default();

    for (type_name, _field_defs) in struct_defs {
        let type_str = encode_type(&struct_types, struct_defs, type_name)?;
        res.insert(type_name.to_owned(), type_str);
    }

    Ok(res)
}

pub fn encode_data(
    schema: &TypeSchema,
    struct_types: &BTreeMap<String, String>,
    data: &mut impl Iterator<Item = Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let res = match schema {
        TypeSchema::Primitive { name, size } => {
            let raw = data.next().ok_or("build value data.next failed")?;
            match name.as_str() {
                "bool" => {
                    let b = raw[0] == 1;
                    b.abi_encode()
                }
                "int" => {
                    if size.is_none() {
                        return Err("size info lacked".into());
                    }
                    let size = size.unwrap() as usize;
                    if raw.len() <= 16 && size <= 16 {
                        let val = parse_i128(&raw, size)?;
                        val.abi_encode()
                    } else {
                        let val = parse_i256(&raw, size)?;
                        val.abi_encode()
                    }
                }
                "uint" => {
                    if size.is_none() {
                        return Err("size info lacked".into());
                    }
                    if raw.len() <= 16 {
                        let val = parse_u128(&raw)?;
                        val.abi_encode()
                    } else {
                        let val = parse_u256(&raw)?;
                        val.abi_encode()
                    }
                }
                "address" => {
                    if raw.len() != 20 {
                        return Err("invalid address len".into());
                    }
                    let addr = Address::from_slice(&raw);
                    addr.abi_encode()
                }
                "bytes" => {
                    if let Some(s) = size {
                        if raw.len() != *s as usize {
                            return Err("invalid fixed bytes len".into());
                        }
                        let fixed_b = Bytes::copy_from_slice(&raw);
                        fixed_b.abi_encode()
                    } else {
                        keccak256(raw).to_vec()
                    }
                }
                "string" => keccak256(raw).to_vec(),
                _ => unreachable!(),
            }
        }
        TypeSchema::Array { item } => {
            let len_v = data.next().ok_or("build value data.next failed")?;
            if len_v.len() != 1 {
                return Err("invalid array size len".to_string());
            }
            let len = len_v[0];
            let mut arr = vec![];

            for _ in 0..len {
                let mut tmp_value = encode_data(item, struct_types, data)?;

                if let TypeSchema::Struct { name, fields: _ } = item.as_ref() {
                    let type_str = struct_types.get(name).ok_or("not found")?;
                    tmp_value = hash_struct(type_str, &tmp_value).to_vec();
                }
                arr.extend(tmp_value);
            }

            keccak256(arr).to_vec()
        }
        TypeSchema::Struct { name: _, fields } => {
            let mut encoded_data = vec![];
            for f in fields {
                let mut f_data = encode_data(&f.ty, struct_types, data)?;

                if let TypeSchema::Struct { name, fields: _ } = &f.ty {
                    let type_str = struct_types.get(name).ok_or("not found")?;
                    f_data = hash_struct(type_str, &f_data).to_vec();
                }

                encoded_data.extend(f_data);
            }

            encoded_data
        }
    };
    Ok(res)
}

pub fn hash_struct(type_str: &String, encoded_data: &Vec<u8>) -> B256 {
    let type_hash = keccak256(type_str.as_bytes());
    let mut hasher = alloy_primitives::Keccak256::new();
    hasher.update(type_hash);
    hasher.update(encoded_data);
    hasher.finalize()
}

pub fn eip712_signing_hash(
    struct_defs: &Eip712StructDefinitions,
    data: &mut impl Iterator<Item = Vec<u8>>,
    primary_type: &String,
    domain: &Eip712Domain,
) -> Result<B256, String> {
    let domain_separator = domain.separator();

    let struct_types = encode_all_struct_type(struct_defs)?;
    let schema = build_schema(struct_defs, primary_type)?;

    let type_str = struct_types.get(primary_type).ok_or("type str not found")?;
    let encoded_data = encode_data(&schema, &struct_types, data)?;
    let struct_hash = hash_struct(type_str, &encoded_data);

    let mut buf = [0u8; 66];
    buf[0] = 0x19;
    buf[1] = 0x01;
    buf[2..34].copy_from_slice(domain_separator.as_slice());
    buf[34..].copy_from_slice(struct_hash.as_slice());

    Ok(keccak256(buf))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use alloc::collections::BTreeMap;
    use alloy_dyn_abi::TypedData;
    use alloy_primitives::hex;

    #[test]
    fn test_encode_type_basic() {
        let struct_defs = prepare_mail_struct_defs();

        let struct_types = encode_types_without_sub_type(&struct_defs).expect("success");
        let type_str = encode_type(&struct_types, &struct_defs, &"Mail".to_string());
        assert!(type_str.is_ok());
        let type_str = type_str.unwrap();
        assert_eq!(
            type_str,
            "Mail(Person from,Person to,string contents,uint64 timestamp,uint256 amount,uint256 payback)Person(string name,address[] wallets)"
        );

        let types2 = encode_all_struct_type(&struct_defs);
        assert!(types2.is_ok());
        let types2 = types2.unwrap();
        assert_eq!(types2.keys().len(), 3);
    }

    #[test]
    fn test_encode_type_arra() {
        let mut struct_defs: Eip712StructDefinitions = Default::default();

        struct_defs.insert(
            "Mail".to_string(),
            vec![
                Eip712FieldDefinition {
                    name: "from".to_string(),
                    field_type: Eip712FieldType::Address,
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "to".to_string(),
                    field_type: Eip712FieldType::Address,
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "contents".to_string(),
                    field_type: Eip712FieldType::String,
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "cc".to_string(),
                    field_type: Eip712FieldType::String,
                    array_levels: vec![Eip712ArrayLevel::Dynamic],
                },
                Eip712FieldDefinition {
                    name: "cc2".to_string(),
                    field_type: Eip712FieldType::String,
                    array_levels: vec![Eip712ArrayLevel::Dynamic, Eip712ArrayLevel::Dynamic],
                },
                Eip712FieldDefinition {
                    name: "cc3".to_string(),
                    field_type: Eip712FieldType::String,
                    array_levels: vec![
                        Eip712ArrayLevel::Dynamic,
                        Eip712ArrayLevel::Dynamic,
                        Eip712ArrayLevel::Fixed(2),
                    ],
                },
            ],
        );

        let struct_types = encode_types_without_sub_type(&struct_defs).expect("success");
        let type_str = encode_type(&struct_types, &struct_defs, &"Mail".to_string());
        assert!(type_str.is_ok());
        let type_str = type_str.unwrap();
        assert_eq!(
            type_str,
            "Mail(address from,address to,string contents,string[] cc,string[][] cc2,string[][][2] cc3)"
        );
    }

    #[test]
    fn test_encode_type_complex_struct() {
        let mut struct_defs: Eip712StructDefinitions = Default::default();

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
                    name: "resume".to_string(),
                    field_type: Eip712FieldType::Custom("File".to_string()),
                    array_levels: vec![],
                },
            ],
        );

        struct_defs.insert(
            "File".to_string(),
            vec![Eip712FieldDefinition {
                name: "name".to_string(),
                field_type: Eip712FieldType::String,
                array_levels: vec![],
            }],
        );

        let struct_types = encode_types_without_sub_type(&struct_defs).expect("success");
        let type_str = encode_type(&struct_types, &struct_defs, &"Mail".to_string());
        assert!(type_str.is_ok());
        let type_str = type_str.unwrap();
        assert_eq!(
            type_str,
            "Mail(Person from,Person to,string contents)File(string name)Person(string name,File resume)"
        );
    }

    #[test]
    fn test_encode_data_basic() {
        let typed_data = get_raw_mail_typed_data().unwrap();
        let typed_data_hash = typed_data.eip712_signing_hash().expect("success");

        let struct_defs = prepare_mail_struct_defs();
        let mail_data = prepare_mail_data();
        let primary_name = "Mail".to_string();

        let schema = build_schema(&struct_defs, &primary_name).unwrap();

        let struct_types = encode_types_without_sub_type(&struct_defs).expect("success");
        let mail_type_str =
            encode_type(&struct_types, &struct_defs, &primary_name).expect("success");

        // check encode type is right
        assert_eq!(mail_type_str, typed_data.encode_type().unwrap());

        let struct_type_map: BTreeMap<String, String> =
            encode_all_struct_type(&struct_defs).expect("success");
        // check encode_data is correct
        let encoded_data = encode_data(&schema, &struct_type_map, &mut mail_data.into_iter());
        assert!(encoded_data.is_ok());

        assert_eq!(
            hex::encode(encoded_data.unwrap()),
            hex::encode(typed_data.encode_data().unwrap())
        );

        // check eip712 hash is match
        let mail_data = prepare_mail_data();

        let maybe_hash = eip712_signing_hash(
            &struct_defs,
            &mut mail_data.into_iter(),
            &primary_name,
            typed_data.domain(),
        );
        assert!(maybe_hash.is_ok());

        assert_eq!(maybe_hash.unwrap(), typed_data_hash);
    }

    fn get_sign_typed_data() -> TypedData {
        let json = r#"
            {
                "domain": {
                    "chainId": 1,
                    "name": "Signed Ints test",
                    "verifyingContract": "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC",
                    "version": "1"
                },
                "message": {
                    "neg256" : "-256",
                    "pos256" : "256",
                    "neg128" : "-128",
                    "pos128" : "128",
                    "neg64" : "-64",
                    "pos64" : "64",
                    "neg32" : "-32",
                    "pos32" : "32",
                    "neg16" : "-16",
                    "pos16" : "16",
                    "neg8" : "-8",
                    "pos8" : "8"
                },
                "primaryType": "Test",
                "types": {
                    "EIP712Domain": [
                        { "name": "name", "type": "string" },
                        { "name": "version", "type": "string" },
                        { "name": "chainId", "type": "uint256" },
                        { "name": "verifyingContract", "type": "address" }
                    ],
                    "Test": [
                        { "name": "neg256", "type": "int256" },
                        { "name": "pos256", "type": "int256" },
                        { "name": "neg128", "type": "int128" },
                        { "name": "pos128", "type": "int128" },
                        { "name": "neg64", "type": "int64" },
                        { "name": "pos64", "type": "int64" },
                        { "name": "neg32", "type": "int32" },
                        { "name": "pos32", "type": "int32" },
                        { "name": "neg16", "type": "int16" },
                        { "name": "pos16", "type": "int16" },
                        { "name": "neg8", "type": "int8" },
                        { "name": "pos8", "type": "int8" }
                    ]
                }
            }
            "#;

        let typed_data: TypedData = serde_json::from_str(json).unwrap();
        typed_data
    }

    #[test]
    fn test_encode_data_sign() {
        let typed_data = get_sign_typed_data();

        let mut struct_defs: Eip712StructDefinitions = Default::default();

        struct_defs.insert(
            "Test".to_string(),
            vec![
                Eip712FieldDefinition {
                    name: "neg256".to_string(),
                    field_type: Eip712FieldType::Int(32),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos256".to_string(),
                    field_type: Eip712FieldType::Int(32),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "neg128".to_string(),
                    field_type: Eip712FieldType::Int(16),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos128".to_string(),
                    field_type: Eip712FieldType::Int(16),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "neg64".to_string(),
                    field_type: Eip712FieldType::Int(8),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos64".to_string(),
                    field_type: Eip712FieldType::Int(8),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "neg32".to_string(),
                    field_type: Eip712FieldType::Int(4),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos32".to_string(),
                    field_type: Eip712FieldType::Int(4),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "neg16".to_string(),
                    field_type: Eip712FieldType::Int(2),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos16".to_string(),
                    field_type: Eip712FieldType::Int(2),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "neg8".to_string(),
                    field_type: Eip712FieldType::Int(1),
                    array_levels: vec![],
                },
                Eip712FieldDefinition {
                    name: "pos8".to_string(),
                    field_type: Eip712FieldType::Int(1),
                    array_levels: vec![],
                },
            ],
        );

        let raw_data = vec![
            hex::decode("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00")
                .unwrap(),
            hex::decode("0100").unwrap(),
            hex::decode("ffffffffffffffffffffffffffffff80").unwrap(),
            hex::decode("80").unwrap(),
            hex::decode("ffffffffffffffc0").unwrap(),
            hex::decode("40").unwrap(),
            hex::decode("ffffffe0").unwrap(),
            hex::decode("20").unwrap(),
            hex::decode("fff0").unwrap(),
            hex::decode("10").unwrap(),
            hex::decode("f8").unwrap(),
            hex::decode("08").unwrap(),
        ];

        let schema = build_schema(&struct_defs, &"Test".to_string()).unwrap();

        let struct_type_map: BTreeMap<String, String> =
            encode_all_struct_type(&struct_defs).expect("success");
        // check encode_data is correct
        let encoded_data = encode_data(&schema, &struct_type_map, &mut raw_data.into_iter());
        assert!(encoded_data.is_ok());
        assert_eq!(
            hex::encode(encoded_data.unwrap()),
            hex::encode(typed_data.encode_data().unwrap())
        );
    }
}
