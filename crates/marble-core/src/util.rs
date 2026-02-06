use std::sync::Arc;

use cel::{ExecutionError, Value, objects::ValueType};

pub fn convert_vec_f32(value: Value) -> Result<Vec<f32>, ExecutionError> {
    Ok(convert_vec_f64(value)?
        .into_iter()
        .map(|v| v as f32)
        .collect())
}

pub fn convert_vec_f64(value: Value) -> Result<Vec<f64>, ExecutionError> {
    match value {
        Value::List(list) => {
            let mut result = Vec::with_capacity(list.len());
            for item in list.iter() {
                result.push(convert_f64(item)?);
            }
            Ok(result)
        }
        _ => Err(value.error_expected_type(ValueType::List)),
    }
}

pub fn convert_f32<A: Into<Value>>(value: A) -> Result<f32, ExecutionError> {
    let value = value.into();
    match value {
        Value::Float(scalar) => Ok(scalar as f32),
        value => Err(value.error_expected_type(ValueType::Float)),
    }
}

pub fn convert_f64<A: Into<Value>>(value: A) -> Result<f64, ExecutionError> {
    let value = value.into();
    match value {
        Value::Float(scalar) => Ok(scalar),
        value => Err(value.error_expected_type(ValueType::Float)),
    }
}

pub fn convert_u64(value: &Value) -> Result<u64, ExecutionError> {
    match value {
        Value::UInt(scalar) => Ok(*scalar),
        value => Err(value.error_expected_type(ValueType::UInt)),
    }
}

pub fn convert_str(value: &Value) -> Result<Arc<String>, ExecutionError> {
    match value {
        Value::String(scalar) => Ok(scalar.clone()),
        value => Err(value.error_expected_type(ValueType::String)),
    }
}

pub fn object_ref<'a, 'b>(
    value: &'a Value,
    refs: &'b [&'static str],
) -> Result<&'a Value, ExecutionError> {
    if refs.is_empty() {
        return Ok(value);
    }
    match value {
        Value::Map(map) => {
            let key = cel::objects::Key::from(refs[0]);
            let next_value = map
                .get(&key)
                .ok_or_else(|| ExecutionError::NoSuchKey(Arc::new(refs[0].to_string())))?;
            object_ref(next_value, &refs[1..])
        }
        _ => Err(value.error_expected_type(ValueType::Map)),
    }
}

pub fn object_ref_or<'a, 'b>(
    value: &'a Value,
    refs: &'b [&'static str],
) -> Result<Option<&'a Value>, ExecutionError> {
    if refs.is_empty() {
        return Ok(Some(value));
    }
    match value {
        Value::Map(map) => {
            let key = cel::objects::Key::from(refs[0]);
            let Some(next_value) = map.get(&key) else {
                return Ok(None);
            };
            object_ref_or(next_value, &refs[1..])
        }
        _ => Err(value.error_expected_type(ValueType::Map)),
    }
}
