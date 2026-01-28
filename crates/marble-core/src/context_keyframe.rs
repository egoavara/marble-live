use std::{collections::HashMap, hash::Hash, sync::Arc};

use cel_interpreter::{Context, Value};

pub struct ContextKeyframe {
    pub name: String,
    pub count: u64,
}
impl ContextKeyframe {
    pub fn new(name: String, count: u64) -> Self {
        Self { name, count }
    }

    pub fn setup_context(&self, target_val: &mut HashMap<Arc<String>, Value>) {
        let mut keyframe = HashMap::<Arc<String>, Value>::new();
        keyframe.insert(Arc::new("name".to_string()), Value::from(self.name.clone()));
        keyframe.insert(
            Arc::new("count".to_string()),
            Value::from(self.count as i64),
        );
        target_val.insert(
            Arc::new("keyframe".to_string()),
            Value::try_from(keyframe).unwrap(),
        );
    }

    pub fn setup_function<'b>(&self, ctx: &'b mut Context<'b>) {}
}
