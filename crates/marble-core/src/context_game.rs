use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use cel_interpreter::{Context, ExecutionError, Value, extractors::This};
use rand::{Rng, SeedableRng};

use crate::util::{convert_str, convert_u64, object_ref, object_ref_or};

#[derive(Debug)]
pub struct ContextGame {
    frame: u64,
    time: f32,
    seed: u64,
}

impl ContextGame {
    pub fn new(seed: u64) -> Self {
        Self {
            frame: 0,
            time: 0.0,
            seed,
        }
    }

    pub fn setup_context(&self, target_val: &mut HashMap<Arc<String>, Value>) {
        target_val.insert(
            Arc::new("frame".to_string()),
            Value::from(self.frame as i64),
        );
        target_val.insert(Arc::new("time".to_string()), Value::from(self.time as f64));
        target_val.insert(Arc::new("seed".to_string()), Value::from(self.seed as u64));
    }

    pub fn setup_function<'b>(&self, ctx: &'b mut Context<'b>) {
        ctx.add_function("random", random);
    }
}

fn random(This(game): This<Value>, min: f64, max: f64) -> Result<f64, ExecutionError> {
    let seed = {
        let ref_seed = object_ref(&game, &["seed"])?;
        let ref_opt_keyframe_name = object_ref_or(&game, &["keyframe", "name"])?;
        let ref_opt_keyframe_count = object_ref_or(&game, &["keyframe", "count"])?;

        let mut seed = convert_u64(ref_seed)? as u64;
        let keyframe_name = ref_opt_keyframe_name.map(|x| convert_str(x)).transpose()?;
        let keyframe_count = ref_opt_keyframe_count.map(|x| convert_u64(x)).transpose()?;

        if let Some(keyframe_name) = keyframe_name {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            keyframe_name.hash(&mut hasher);
            if let Some(count) = keyframe_count {
                count.hash(&mut hasher);
            }
            seed ^= hasher.finish()
        }
        if let Some(count) = keyframe_count {
            seed ^= count
        }
        seed
    };
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let value = rng.random_range(min..max);
    Ok(value)
}
