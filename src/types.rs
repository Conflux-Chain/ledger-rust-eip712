use crate::utils::{parse_u64, parse_utf8_string};
use alloc::{
    collections::BTreeMap,
    string::{String, ToString},
    vec,
    vec::Vec,
    format,
};
use alloy_dyn_abi::{Eip712Types, PropertyDef, Resolver};
use alloy_primitives::hex;
use bytes::{Buf, Bytes, TryGetError};

pub const EIP712_DOMAIN_TYPE_NAME: &'static str = "EIP712Domain";

/// EIP-712 field type enumeration
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Eip712FieldType {
    /// Custom struct type
    Custom(String),
    /// Integer type with size in bytes
    Int(u8),
    /// Unsigned integer type with size in bytes
    Uint(u8),
    /// Ethereum address type
    Address,
    /// Boolean type
    Bool,
    /// String type
    String,
    /// Fixed-size bytes with size
    FixedBytes(u8),
    /// Dynamic-size bytes
    DynamicBytes,
}

impl Eip712FieldType {
    /// Get the type ID for encoding
    pub fn type_id(&self) -> u8 {
        match self {
            Eip712FieldType::Custom(_) => 0,
            Eip712FieldType::Int(_) => 1,
            Eip712FieldType::Uint(_) => 2,
            Eip712FieldType::Address => 3,
            Eip712FieldType::Bool => 4,
            Eip712FieldType::String => 5,
            Eip712FieldType::FixedBytes(_) => 6,
            Eip712FieldType::DynamicBytes => 7,
        }
    }

    /// Get the type size if applicable
    pub fn type_size(&self) -> Option<u8> {
        match self {
            Eip712FieldType::Int(size) => Some(*size),
            Eip712FieldType::Uint(size) => Some(*size),
            Eip712FieldType::FixedBytes(size) => Some(*size),
            _ => None,
        }
    }

    /// Get the type name for custom types
    pub fn custom_type_name(&self) -> Option<&str> {
        match self {
            Eip712FieldType::Custom(name) => Some(name),
            _ => None,
        }
    }

    pub fn type_string(&self) -> String {
        match self {
            Eip712FieldType::Custom(name) => name.clone(),
            Eip712FieldType::Int(size) => format!("int{}", *size as usize * 8),
            Eip712FieldType::Uint(size) => format!("uint{}", *size as usize * 8),
            Eip712FieldType::Address => "address".to_string(),
            Eip712FieldType::Bool => "bool".to_string(),
            Eip712FieldType::String => "string".to_string(),
            Eip712FieldType::FixedBytes(size) => format!("bytes{}", size),
            Eip712FieldType::DynamicBytes => "bytes".to_string(),
        }
    }

    pub fn type_string_and_size(&self) -> (String, Option<u8>) {
        let name = match self {
            Eip712FieldType::Custom(name) => name.clone(),
            Eip712FieldType::Int(_) => "int".to_string(),
            Eip712FieldType::Uint(_) => "uint".to_string(),
            Eip712FieldType::Address => "address".to_string(),
            Eip712FieldType::Bool => "bool".to_string(),
            Eip712FieldType::String => "string".to_string(),
            Eip712FieldType::FixedBytes(_) => "bytes".to_string(),
            Eip712FieldType::DynamicBytes => "bytes".to_string(),
        };
        (name, self.type_size())
    }
}

/// EIP-712 array level type
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Eip712ArrayLevel {
    /// Dynamic array (type[])
    Dynamic,
    /// Fixed-size array (type[N]) u8 means the array size can not bigger than 256
    Fixed(u8),
}

impl Eip712ArrayLevel {
    /// Get the array level type ID for encoding
    pub fn type_id(&self) -> u8 {
        match self {
            Eip712ArrayLevel::Dynamic => 0,
            Eip712ArrayLevel::Fixed(_) => 1,
        }
    }

    /// Get the array size if fixed
    pub fn size(&self) -> Option<u8> {
        match self {
            Eip712ArrayLevel::Fixed(size) => Some(*size),
            Eip712ArrayLevel::Dynamic => None,
        }
    }

    pub fn type_string(&self) -> String {
        match self {
            Eip712ArrayLevel::Dynamic => "[]".to_string(),
            Eip712ArrayLevel::Fixed(size) => format!("[{}]", size),
        }
    }
}

/// EIP-712 struct field definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip712FieldDefinition {
    /// Field data type
    pub field_type: Eip712FieldType,
    /// Field name
    pub name: String,
    /// Array levels (empty if not an array)
    pub array_levels: Vec<Eip712ArrayLevel>,
}

impl Eip712FieldDefinition {
    /// Create a new field definition
    pub fn new(field_type: Eip712FieldType, name: String) -> Self {
        Eip712FieldDefinition {
            field_type,
            name,
            array_levels: Vec::new(),
        }
    }

    pub fn is_primitive(&self) -> bool {
        return !self.is_array() && !self.is_struct();
    }

    pub fn is_struct(&self) -> bool {
        matches!(self.field_type, Eip712FieldType::Custom(_))
    }

    /// Add an array level to the field
    pub fn with_array_level(mut self, level: Eip712ArrayLevel) -> Self {
        self.array_levels.push(level);
        self
    }

    /// Check if this field is an array
    pub fn is_array(&self) -> bool {
        !self.array_levels.is_empty()
    }

    pub fn type_string(&self) -> String {
        let mut type_str = self.field_type.type_string();
        for level in &self.array_levels {
            type_str.push_str(&level.type_string());
        }
        type_str
    }

    pub fn primitive_type_string_and_size(&self) -> (String, Option<u8>) {
        self.field_type.type_string_and_size()
    }

    pub fn to_proper_def(&self) -> Result<PropertyDef, &'static str> {
        PropertyDef::new(self.type_string(), self.name.clone()).map_err(|_| "invalid type")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let get_err_str = |_: TryGetError| "Invalid len";

        let mut buf = Bytes::copy_from_slice(bytes);

        // decode type info
        let type_desc = buf.try_get_u8().map_err(get_err_str)?;
        let is_array = (type_desc & 0x80) == 0x80;
        let is_type_size_specified = (type_desc & 0x40) == 0x40;
        let field_type_id = type_desc & 0x0F;

        let field_type = match field_type_id {
            0 => {
                let custom_name_len = buf.try_get_u8().map_err(get_err_str)? as usize;
                if buf.remaining() < custom_name_len {
                    return Err("Unexpected end of input when reading custom name");
                }
                let mut custom_name_bytes = vec![0u8; custom_name_len];
                buf.copy_to_slice(&mut custom_name_bytes);
                let custom_name = parse_utf8_string(&custom_name_bytes)?;
                Eip712FieldType::Custom(custom_name)
            }
            1 => {
                if !is_type_size_specified {
                    return Err("Int type must specify size");
                }
                let type_size = buf.try_get_u8().map_err(get_err_str)?;
                Eip712FieldType::Int(type_size)
            }
            2 => {
                if !is_type_size_specified {
                    return Err("Int type must specify size");
                }
                let type_size = buf.try_get_u8().map_err(get_err_str)?;
                Eip712FieldType::Uint(type_size)
            }
            3 => Eip712FieldType::Address,
            4 => Eip712FieldType::Bool,
            5 => Eip712FieldType::String,
            6 => {
                if !is_type_size_specified {
                    return Err("Int type must specify size");
                }
                let type_size = buf.try_get_u8().map_err(get_err_str)?;
                Eip712FieldType::FixedBytes(type_size)
            }
            7 => Eip712FieldType::DynamicBytes,
            _ => return Err("Unknown field type"),
        };

        // decode array levels info
        let array_levels = if is_array {
            let mut levels = Vec::new();
            let level_count = buf.try_get_u8().map_err(get_err_str)? as usize;
            for _ in 0..level_count {
                let level_desc = buf.try_get_u8().map_err(get_err_str)?;

                match level_desc {
                    0 => levels.push(Eip712ArrayLevel::Dynamic),
                    1 => {
                        let size = buf.try_get_u8().map_err(get_err_str)?;
                        levels.push(Eip712ArrayLevel::Fixed(size));
                    }
                    _ => return Err("Unknown array level type"),
                }
            }
            levels
        } else {
            Vec::new()
        };

        // decode field name
        let name_len = buf.try_get_u8().map_err(get_err_str)? as usize;
        if buf.remaining() < name_len {
            return Err("Unexpected end of input when reading field name");
        }
        let mut name_bytes = vec![0u8; name_len];
        buf.copy_to_slice(&mut name_bytes);
        let name = parse_utf8_string(&name_bytes)?;

        Ok(Eip712FieldDefinition {
            field_type,
            name,
            array_levels,
        })
    }
}

/// EIP-712 struct definition
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip712StructDefinition {
    /// Struct name
    pub name: String,
    /// Struct fields
    pub fields: Vec<Eip712FieldDefinition>,
}

pub type Eip712StructDefinitions = BTreeMap<String, Vec<Eip712FieldDefinition>>;

pub fn build_resolver_from_struct_defs(
    struct_defs: &Eip712StructDefinitions,
) -> Result<Resolver, &'static str> {
    let mut eip712_types: Eip712Types = Default::default();
    for (name, defs) in struct_defs.iter() {
        let mut property_defs = Vec::new();
        for field in defs {
            let property_def = field.to_proper_def().unwrap();
            property_defs.push(property_def);
        }
        eip712_types.insert(name.clone(), property_defs);
    }
    let resolver = Resolver::from(eip712_types);
    Ok(resolver)
}

/// EIP-712 struct implementation value
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip712FieldValue {
    /// Raw value data
    pub value: Vec<u8>,
}

impl Eip712FieldValue {
    /// Create a new field value
    pub fn new(value: Vec<u8>) -> Self {
        Eip712FieldValue { value }
    }

    /// Create from a string value
    pub fn from_string(s: &str) -> Self {
        Eip712FieldValue {
            value: s.as_bytes().to_vec(),
        }
    }

    pub fn to_string(self) -> Result<String, &'static str> {
        String::from_utf8(self.value).map_err(|_| "invalid data")
    }

    /// Create from a u256 value
    pub fn from_u256(value: &[u8; 32]) -> Self {
        Eip712FieldValue {
            value: value.to_vec(),
        }
    }

    /// Create from an address
    pub fn from_address(address: &[u8; 20]) -> Self {
        Eip712FieldValue {
            value: address.to_vec(),
        }
    }

    /// Create from a boolean
    pub fn from_bool(value: bool) -> Self {
        Eip712FieldValue {
            value: vec![if value { 1 } else { 0 }],
        }
    }

    /// Create from a uint value (defaults to 8-byte u64)
    pub fn from_uint(value: u64) -> Self {
        Eip712FieldValue {
            value: value.to_be_bytes().to_vec(),
        }
    }

    /// Create from a uint value with specific size
    pub fn from_uint_sized(size: u8, value: u64) -> Self {
        let mut bytes = vec![0u8; size as usize];
        let value_bytes = value.to_be_bytes();
        let start = bytes.len().saturating_sub(value_bytes.len());
        let copy_len = (bytes.len() - start).min(value_bytes.len());
        bytes[start..start + copy_len]
            .copy_from_slice(&value_bytes[value_bytes.len() - copy_len..]);
        Eip712FieldValue { value: bytes }
    }

    pub fn to_u64(self) -> Result<u64, &'static str> {
        parse_u64(&self.value)
    }

    /// Create from a uint32 value (4 bytes)
    pub fn from_uint32(value: u32) -> Self {
        Eip712FieldValue {
            value: value.to_be_bytes().to_vec(),
        }
    }

    /// Create from an address string (hex format)
    pub fn from_address_str(address: &str) -> Result<Self, String> {
        // Remove 0x prefix if present
        let hex_str = if let Some(stripped) = address.strip_prefix("0x") {
            stripped
        } else {
            address
        };

        // Validate length
        if hex_str.len() != 40 {
            return Err(format!(
                "Invalid address length: expected 40 hex characters, got {}",
                hex_str.len()
            ));
        }

        // Parse hex
        let bytes = hex::decode(hex_str).map_err(|e| format!("Invalid hex: {}", e))?;
        if bytes.len() != 20 {
            return Err("Address must be 20 bytes".to_string());
        }

        Ok(Eip712FieldValue { value: bytes })
    }

    pub fn to_address_string(&self) -> Result<String, &str> {
        if self.value.len() != 20 {
            return Err("invalid len");
        }
        let mut hex_addr = String::from("0x");
        hex_addr.push_str(&hex::encode(&self.value));
        Ok(hex_addr)
    }

    /// Create a reference to a nested struct (empty value for struct references)
    pub fn from_struct() -> Self {
        Eip712FieldValue { value: vec![] }
    }

    /// Create from an int value with specific size
    pub fn from_int_sized(size: u8, value: i64) -> Self {
        let mut bytes = vec![0u8; size as usize];
        let value_bytes = value.to_be_bytes();
        let start = bytes.len().saturating_sub(value_bytes.len());
        let copy_len = (bytes.len() - start).min(value_bytes.len());
        bytes[start..start + copy_len]
            .copy_from_slice(&value_bytes[value_bytes.len() - copy_len..]);
        Eip712FieldValue { value: bytes }
    }

    /// Create from bytes
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Eip712FieldValue { value: bytes }
    }
}

/// EIP-712 struct implementation
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip712StructImplementation {
    /// Struct name
    pub name: String,
    /// Field values in order
    pub values: Vec<Eip712FieldValue>,
}

impl Eip712StructImplementation {
    /// Create a new struct implementation
    pub fn new(name: String) -> Self {
        Eip712StructImplementation {
            name,
            values: Vec::new(),
        }
    }

    /// Add a field value
    pub fn with_value(mut self, value: Eip712FieldValue) -> Self {
        self.values.push(value);
        self
    }
}

pub type Eip712StructImplementations = BTreeMap<String, Vec<Eip712FieldValue>>;

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::{Eip712ArrayLevel, Eip712FieldDefinition, Eip712FieldType};
    use alloy_primitives::hex;

    #[test]
    fn test_field_definition_from_types_eip712_doamin_type() {
        // eipdomain.name
        let data = hex::decode("05046e616d65").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "name");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::String);
        // eipdomain.version
        let data = hex::decode("050776657273696f6e").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "version");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::String);
        // eipdomain.chainId
        let data = hex::decode("422007636861696e4964").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "chainId");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Uint(32));
        // eipdomain.verifyingContract
        let data = hex::decode("0311766572696679696e67436f6e7472616374").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "verifyingContract");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Address);
    }

    #[test]
    fn test_field_definition_from_types_uint() {
        let data = hex::decode("412006696e74323536").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "int256");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Int(32));

        let data = hex::decode("411006696e74313238").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "int128");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Int(16));

        let data = hex::decode("410805696e743634").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "int64");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Int(8));

        let data = hex::decode("410104696e7438").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "int8");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Int(1));

        let data = hex::decode("42100775696e74313238").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "uint128");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Uint(16));

        let data = hex::decode("42080675696e743634").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "uint64");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Uint(8));

        let data = hex::decode("42010575696e7438").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "uint8");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Uint(1));
    }

    #[test]
    fn test_field_definition_from_types_bool() {
        let data = hex::decode("0404626f6f6c").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bool");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::Bool);
    }

    #[test]
    fn test_field_definition_from_types_array() {
        let data = hex::decode("8006506572736f6e0100026363").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "cc");
        assert_eq!(field_def.array_levels.len(), 1);
        assert_eq!(
            field_def.field_type,
            Eip712FieldType::Custom("Person".to_string())
        );

        let data = hex::decode("84010008626f6f6c5f617272").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bool_arr");
        assert_eq!(field_def.array_levels.len(), 1);
        assert_eq!(field_def.field_type, Eip712FieldType::Bool);

        let data = hex::decode("8402000009626f6f6c5f61727232").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bool_arr2");
        assert_eq!(field_def.array_levels.len(), 2);
        assert_eq!(field_def.field_type, Eip712FieldType::Bool);

        let data = hex::decode("84020001020f626f6f6c5f617272325f6669786564").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bool_arr2_fixed");
        assert_eq!(field_def.array_levels.len(), 2);
        assert_eq!(field_def.field_type, Eip712FieldType::Bool);
        assert_eq!(field_def.array_levels[0], Eip712ArrayLevel::Dynamic);
        assert_eq!(field_def.array_levels[1], Eip712ArrayLevel::Fixed(2));
    }

    #[test]
    fn test_field_definition_from_types_custom() {
        let data = hex::decode("0006506572736f6e0466726f6d").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "from");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(
            field_def.field_type,
            Eip712FieldType::Custom("Person".to_string())
        );
    }

    #[test]
    fn test_field_definition_from_types_bytes() {
        let data = hex::decode("07056279746573").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bytes");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::DynamicBytes);

        let data = hex::decode("460106627974657331").expect("success");
        let field_def = Eip712FieldDefinition::from_bytes(&data).expect("success");
        assert_eq!(field_def.name, "bytes1");
        assert_eq!(field_def.array_levels.len(), 0);
        assert_eq!(field_def.field_type, Eip712FieldType::FixedBytes(1));
    }
}
