// Test-only checks that serialized types and served routes match openapi.json.
// The checks' own tests live in api.rs, beside the routes the spec documents.

use arrayvec::ArrayVec;
use serde::Serialize;
use serde_json::Value;

const OPENAPI_SPEC: &str = include_str!("../../openapi.json");
const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";
const JSON_TYPE_COUNT: usize = 7;

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

// `type` is either one name or a list of names (3.1 nullable form), borrowed from the spec.
fn declared_types(property: &Value) -> ArrayVec<&str, JSON_TYPE_COUNT> {
    let declared: &Value = &property["type"];
    if let Some(name) = declared.as_str() {
        return [name].into_iter().collect();
    }
    let Some(names) = declared.as_array() else {
        panic!("schema property declares no type: {property}");
    };
    assert!(names.len() <= JSON_TYPE_COUNT, "JSON Schema has seven types, so a longer list is a malformed spec: {property}");
    return names.iter().filter_map(Value::as_str).collect();
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
    // serde_json::Map is a BTreeMap here (no preserve_order feature), so both key lists iterate sorted.
    assert!(object.keys().eq(properties.keys()), "{schema_name}: serialized keys differ from spec properties");
    let required: &[Value] = object_schema["required"].as_array().map_or(&[], |names| names.as_slice());
    // `required` names are unique in JSON Schema, so equal counts and containment give set equality.
    let is_all_required: bool = required.len() == properties.len()
        && required.iter().all(|name: &Value| name.as_str().is_some_and(|n: &str| properties.contains_key(n)));
    assert!(is_all_required, "{schema_name}: every serialized field is always present");
    object.iter().for_each(|(key, value): (&String, &Value)| {
        let property: &Value = resolve(spec, &properties[key]);
        let allowed: ArrayVec<&str, JSON_TYPE_COUNT> = declared_types(property);
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
    let documented: &[Value] = schema(spec, schema_name)["enum"].as_array().map_or(&[], |members| members.as_slice());
    let is_documented = |variant: &T| serde_json::to_value(variant).is_ok_and(|v: Value| documented.contains(&v));
    let is_variant = |member: &Value| {
        return member.is_null() || variants.iter().any(|v: &T| serde_json::to_value(v).is_ok_and(|s: Value| s == *member));
    };
    let is_match: bool = documented.len() == variants.len() + 1
        && documented.contains(&Value::Null)
        && variants.iter().all(is_documented)
        && documented.iter().all(is_variant);
    assert!(is_match, "{schema_name}: spec enum differs from the Rust variants, spec lists {documented:?}");
}
