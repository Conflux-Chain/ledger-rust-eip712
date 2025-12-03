use crate::types::Eip712StructDefinitions;
use crate::utils::*;

use alloc::{
    borrow::ToOwned,
    boxed::Box,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use alloy_primitives::hex;
use serde_json::{Number, Value};

pub enum TypeSchema {
    // type name(uint) and it's possible size, only uint, int, bytes will have size
    Primitive { name: String, size: Option<u8> },
    Array { item: Box<TypeSchema> },
    // Struct name(Person) and its fields
    Struct { name: String, fields: Vec<Field> },
}

pub struct Field {
    // the field name, eg: from, not type
    pub name: String,
    pub ty: TypeSchema,
}

pub fn build_schema(
    struct_defs: &Eip712StructDefinitions,
    type_name: &String,
) -> Result<TypeSchema, String> {
    let field_defs = struct_defs.get(type_name).ok_or("build_schema not found")?;

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

    return Ok(TypeSchema::Struct {
        name: type_name.to_owned(),
        fields,
    });
}

// from type schema and raw data build serde_json::Value
pub fn build_value(
    schema: &TypeSchema,
    data: &mut impl Iterator<Item = Vec<u8>>,
) -> Result<Value, String> {
    let res = match schema {
        TypeSchema::Primitive { name, size } => {
            let raw = data.next().ok_or("build value data.next failed")?;
            match name.as_str() {
                "bool" => Value::Bool(raw[0] == 1),
                "int" => {
                    let the_size = size.expect("exist") as usize;
                    if raw.len() > the_size as usize {
                        return Err("invalid int len".to_string());
                    }
                    if the_size <= 16 {
                        let val = parse_i128(&raw, the_size).map_err(|err| err.to_string())?;
                        match Number::from_i128(val) {
                            Some(num) => Value::Number(num),
                            None => Value::String(format!("{:#x}", val)),
                        }
                    } else {
                        let val = parse_i256(&raw, the_size).map_err(|err| err.to_string())?;
                        Value::String(val.to_hex_string())
                    }
                }
                "uint" => {
                    if let Some(s) = size {
                        if raw.len() > *s as usize {
                            return Err("invalid uint len".to_string());
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
                            return Err("invalid bytes len".to_string());
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
                        return Err("invalid address len".to_string());
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
            let len_v = data.next().ok_or("build value data.next failed")?;
            if len_v.len() != 1 {
                return Err("invalid array size len".to_string());
            }
            let len = len_v[0];
            let mut arr = vec![];

            for _ in 0..len {
                arr.push(build_value(item, data)?);
            }

            arr.into()
        }
        TypeSchema::Struct { name: _, fields } => {
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
            let raw = data.next().ok_or("build_ui data.next failed")?;
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
                    let the_size = size.expect("exist") as usize;
                    if raw.len() > the_size as usize {
                        return Err("invalid int len".to_string());
                    }
                    let value = if the_size <= 16 {
                        let val = parse_i128(&raw, the_size).map_err(|err| err.to_string())?;
                        format!("{}", val)
                    } else {
                        let val = parse_i256(&raw, the_size).map_err(|err| err.to_string())?;
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
                            return Err("invalid uint len".to_string());
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
                            return Err("invalid bytes len".to_string());
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
                        return Err("invalid address len".to_string());
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
            let len_v = data.next().ok_or("build_ui data.next failed")?;
            if len_v.len() != 1 {
                return Err("invalid array size len".to_string());
            }
            let len = len_v[0];
            let mut arr = vec![];

            for _ in 0..len {
                arr.extend(build_ui_fields(item, data, field_name)?);
            }

            arr
        }
        TypeSchema::Struct { name: _, fields } => {
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
    use super::{build_schema, build_ui_fields, build_value};
    use crate::{
        test_utils::*,
        types::{
            Eip712FieldDefinition, Eip712FieldType, Eip712StructDefinitions,
            build_resolver_from_struct_defs,
        },
    };
    use alloy_dyn_abi::eip712::TypedData;
    use alloy_primitives::hex;

    #[test]
    fn test_build_value() {
        let struct_defs = prepare_mail_struct_defs();

        let type_schema = build_schema(&struct_defs, &"Mail".to_string());
        assert_eq!(type_schema.is_ok(), true);
        let type_schema = type_schema.unwrap();

        let data = prepare_mail_data();
        let value = build_value(&type_schema, &mut data.into_iter());
        assert_eq!(value.is_ok(), true);
        let value = value.unwrap();

        let typed = get_raw_mail_typed_data().expect("success");

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
        let struct_defs = prepare_mail_struct_defs();

        let type_schema = build_schema(&struct_defs, &"Mail".to_string());
        assert_eq!(type_schema.is_ok(), true);
        let type_schema = type_schema.unwrap();

        let data = prepare_mail_data();

        let ui_fields = build_ui_fields(&type_schema, &mut data.into_iter(), "");
        assert!(ui_fields.is_ok());
        let ui_fields = ui_fields.unwrap();
        println!("{:?}", ui_fields);
        assert!(ui_fields.len() > 0);
    }

    #[test]
    fn test_signed_int() {
        let mut struct_defs: Eip712StructDefinitions = Default::default();

        struct_defs.insert("EIP712Domain".to_string(), get_domain_struct_def());

        let primary_type = "Test".to_string();

        struct_defs.insert(
            primary_type.clone(),
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

        let data = vec![
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

        let type_schema = build_schema(&struct_defs, &primary_type).unwrap();

        let value = build_value(&type_schema, &mut data.into_iter());
        assert_eq!(value.is_ok(), true);
        let value = value.unwrap();

        let resolver = build_resolver_from_struct_defs(&struct_defs).unwrap();

        let typed = get_raw_mail_typed_data().expect("success");
        let new_typed_data = TypedData {
            domain: typed.domain.clone(),
            resolver,
            primary_type: primary_type,
            message: value,
        };

        let maybe_hash2 = new_typed_data.eip712_signing_hash();
        assert!(maybe_hash2.is_ok());
    }
}
