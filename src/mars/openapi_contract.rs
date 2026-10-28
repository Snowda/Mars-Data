// Test-only checks that serialized types and served routes match openapi.json.
// The checks' own tests live in api.rs, beside the routes the spec documents.

use serde::Serialize;
use serde_json::Value;

const OPENAPI_SPEC: &str = include_str!("../../openapi.json");
const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

pub fn spec() -> Value {
    let Ok(spec) = serde_json::from_str::<Value>(OPENAPI_SPEC) else {
        panic!("openapi.json must parse");
    };
    return spec;
}

pub fn schema<'a>(spec: &'a Value, name: &str) -> &'a Value {
    return &spec["components"]["schemas"][name];
}

// Follows a property's $ref into components.schemas.
fn resolve<'a>(spec: &'a Value, property: &'a Value) -> &'a Value {
    return match property["$ref"].as_str() {
        Some(reference) => schema(spec, reference.trim_start_matches(SCHEMA_REF_PREFIX)),
        None => property,
    };
}

// `type` is either one name or a list of names (3.1 nullable form).
fn declared_types(property: &Value) -> Vec<&str> {
    return match &property["type"] {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names) => names.iter().filter_map(Value::as_str).collect(),
        _ => panic!("schema property declares no type: {property}"),
    };
}

pub fn json_type(value: &Value) -> &'static str {
    return match value {
        Value::String(_) => "string",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    };
}

// Panics unless `instance` has exactly the schema's properties, all required, each of an allowed
// type and (for enums) an allowed value.
pub fn assert_conforms(spec: &Value, schema_name: &str, instance: &Value) {
    let object_schema: &Value = schema(spec, schema_name);
    let Some(properties) = object_schema["properties"].as_object() else {
        panic!("{schema_name}: no properties");
    };
    let Some(object) = instance.as_object() else {
        panic!("{schema_name}: instance is not an object");
    };
    let mut declared: Vec<&str> = properties.keys().map(String::as_str).collect();
    let mut required: Vec<&str> =
        object_schema["required"].as_array().map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let mut actual: Vec<&str> = object.keys().map(String::as_str).collect();
    declared.sort_unstable();
    required.sort_unstable();
    actual.sort_unstable();
    assert_eq!(actual, declared, "{schema_name}: serialized keys differ from spec properties");
    assert_eq!(required, declared, "{schema_name}: every serialized field is always present");
    object.iter().for_each(|(key, value): (&String, &Value)| {
        let property: &Value = resolve(spec, &properties[key]);
        let allowed: Vec<&str> = declared_types(property);
        let got: &str = json_type(value);
        let is_type_match: bool = allowed.contains(&got) || (got == "integer" && allowed.contains(&"number"));
        assert!(is_type_match, "{schema_name}.{key}: spec allows {allowed:?}, serializer emits {got}");
        if let Some(members) = property["enum"].as_array() {
            assert!(members.contains(value), "{schema_name}.{key}: {value} is not in the spec enum");
        }
    });
}

// Panics unless the spec enum lists exactly the serialized `variants`, plus null.
pub fn assert_enum_matches<T: Serialize>(spec: &Value, schema_name: &str, variants: &[T]) {
    let mut documented: Vec<Value> = schema(spec, schema_name)["enum"].as_array().cloned().unwrap_or_default();
    let mut serialized: Vec<Value> =
        variants.iter().filter_map(|v: &T| serde_json::to_value(v).ok()).chain([Value::Null]).collect();
    documented.sort_by_key(Value::to_string);
    serialized.sort_by_key(Value::to_string);
    assert_eq!(serialized, documented, "{schema_name}: spec enum differs from the Rust variants");
}
