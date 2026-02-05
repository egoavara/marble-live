//! CEL DSL types for dynamic map expressions.
//!
//! Provides types that can be either static values or CEL expressions
//! evaluated at runtime.

use std::collections::HashMap;
use std::sync::Arc;

use cel_interpreter::{Context, Program, Value};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

/// Error type for CEL expression evaluation.
#[derive(Debug, thiserror::Error)]
pub enum DslError {
    #[error("CEL compile error: {0}")]
    Compile(String),
    #[error("CEL execution error: {0}")]
    Execution(String),
    #[error("Expected numeric result, got: {0}")]
    TypeMismatch(String),
}

/// A number or CEL expression that evaluates to a number.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum NumberOrExpr {
    Number(f32),
    Expr(String),
}

impl Default for NumberOrExpr {
    fn default() -> Self {
        Self::Number(0.0)
    }
}

impl NumberOrExpr {
    /// Evaluates the expression to an f32 value.
    pub fn evaluate(&self, ctx: &GameContext) -> f32 {
        match self {
            Self::Number(n) => *n,
            Self::Expr(expr) => ctx.eval_f32(expr).unwrap_or(0.0),
        }
    }

    /// Evaluates the expression with random() macro support.
    /// Use this for expressions that may contain random(min, max).
    pub fn evaluate_with_random(&self, ctx: &mut GameContext) -> f32 {
        match self {
            Self::Number(n) => *n,
            Self::Expr(expr) => ctx.eval_f32_with_random(expr).unwrap_or(0.0),
        }
    }

    /// Returns true if this is a dynamic expression (not a static number).
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Expr(_))
    }
}

/// A 2D vector that can be static or dynamic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Vec2OrExpr {
    /// Static [x, y] values.
    Static([f32; 2]),
    /// Dynamic values where each component can be a number or expression.
    Dynamic([NumberOrExpr; 2]),
}

/// A boolean or CEL expression that evaluates to a boolean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum BoolOrExpr {
    Bool(bool),
    Expr(String),
}

impl Default for BoolOrExpr {
    fn default() -> Self {
        Self::Bool(true)
    }
}

impl BoolOrExpr {
    /// Evaluates the expression to a bool value.
    pub fn evaluate(&self, ctx: &GameContext) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Expr(expr) => ctx.eval_bool(expr).unwrap_or(false),
        }
    }

    /// Returns true if this is a dynamic expression.
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Expr(_))
    }
}

impl Vec2OrExpr {
    /// Evaluates to [f32; 2].
    pub fn evaluate(&self, ctx: &GameContext) -> [f32; 2] {
        match self {
            Self::Static(v) => *v,
            Self::Dynamic(v) => [v[0].evaluate(ctx), v[1].evaluate(ctx)],
        }
    }

    /// Returns true if any component is dynamic.
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic(v) if v[0].is_dynamic() || v[1].is_dynamic())
    }
}

/// Game context providing runtime variables for CEL expressions.
#[derive(Debug, Clone)]
pub struct GameContext {
    /// Time elapsed since game start (seconds).
    pub time: f32,
    /// Current frame number.
    pub frame: u64,
    /// Cached compiled programs for expression reuse.
    #[allow(clippy::type_complexity)]
    program_cache: Option<Arc<parking_lot::RwLock<HashMap<String, Arc<Program>>>>>,
    /// RNG for random() macro (deterministic).
    rng: Option<ChaCha8Rng>,
}

impl Default for GameContext {
    fn default() -> Self {
        Self::new(0.0, 0)
    }
}

impl GameContext {
    /// Creates a new game context.
    pub fn new(time: f32, frame: u64) -> Self {
        Self {
            time,
            frame,
            program_cache: None,
            rng: None,
        }
    }

    /// Creates a game context with expression caching enabled.
    pub fn with_cache() -> Self {
        Self {
            time: 0.0,
            frame: 0,
            program_cache: Some(Arc::new(parking_lot::RwLock::new(HashMap::new()))),
            rng: None,
        }
    }

    /// Creates a game context with caching and deterministic RNG for random() macro.
    pub fn with_cache_and_seed(seed: u64) -> Self {
        Self {
            time: 0.0,
            frame: 0,
            program_cache: Some(Arc::new(parking_lot::RwLock::new(HashMap::new()))),
            rng: Some(ChaCha8Rng::seed_from_u64(seed)),
        }
    }

    /// Updates the context with new time and frame values.
    pub fn update(&mut self, time: f32, frame: u64) {
        self.time = time;
        self.frame = frame;
    }

    /// Preprocesses an expression to replace random(min, max) with actual random values.
    /// Returns the processed expression string.
    pub fn preprocess_random(&mut self, expr: &str) -> String {
        let Some(rng) = &mut self.rng else {
            return expr.to_string();
        };

        let mut result = expr.to_string();

        // Find and replace all random(min, max) patterns
        while let Some(start) = result.find("random(") {
            // Find the closing parenthesis
            let after_open = start + 7; // length of "random("
            let Some(rel_close) = result[after_open..].find(')') else {
                break;
            };
            let end = after_open + rel_close;

            // Extract the arguments
            let args_str = &result[after_open..end];
            let parts: Vec<&str> = args_str.split(',').collect();

            if parts.len() == 2 {
                let min: f32 = parts[0].trim().parse().unwrap_or(0.0);
                let max: f32 = parts[1].trim().parse().unwrap_or(1.0);
                let value = rng.random_range(min..max);

                // Replace the random() call with the value
                result = format!("{}{}{}", &result[..start], value, &result[end + 1..]);
            } else {
                break;
            }
        }

        result
    }

    /// Converts to CEL context for expression evaluation.
    fn to_cel_context(&self) -> Context<'_> {
        let mut ctx = Context::default();

        // Create game object with time and frame
        let mut game_map: HashMap<Arc<String>, Value> = HashMap::new();
        game_map.insert(Arc::new("time".to_string()), Value::Float(f64::from(self.time)));
        game_map.insert(Arc::new("frame".to_string()), Value::Int(self.frame as i64));

        ctx.add_variable("game", Value::Map(game_map.into())).ok();
        ctx
    }

    /// Compiles a CEL expression, using cache if available.
    fn compile_expr(&self, expr: &str) -> Result<Arc<Program>, DslError> {
        if let Some(cache) = &self.program_cache {
            // Check cache first
            {
                let read_guard = cache.read();
                if let Some(program) = read_guard.get(expr) {
                    return Ok(Arc::clone(program));
                }
            }

            // Compile and cache
            let program = Arc::new(
                Program::compile(expr).map_err(|e| DslError::Compile(format!("{e:?}")))?,
            );
            cache.write().insert(expr.to_string(), Arc::clone(&program));
            Ok(program)
        } else {
            // No cache, compile directly
            Ok(Arc::new(
                Program::compile(expr).map_err(|e| DslError::Compile(format!("{e:?}")))?,
            ))
        }
    }

    /// Evaluates a CEL expression to an f32.
    pub fn eval_f32(&self, expr: &str) -> Result<f32, DslError> {
        let program = self.compile_expr(expr)?;
        let cel_ctx = self.to_cel_context();
        let value = program
            .execute(&cel_ctx)
            .map_err(|e| DslError::Execution(format!("{e:?}")))?;

        match value {
            Value::Float(f) => Ok(f as f32),
            Value::Int(i) => Ok(i as f32),
            Value::UInt(u) => Ok(u as f32),
            other => Err(DslError::TypeMismatch(format!("{other:?}"))),
        }
    }

    /// Evaluates a CEL expression to an f32, with random() macro support.
    /// Requires mutable access for RNG state.
    pub fn eval_f32_with_random(&mut self, expr: &str) -> Result<f32, DslError> {
        let processed = self.preprocess_random(expr);
        self.eval_f32(&processed)
    }

    /// Evaluates a CEL expression to an f64.
    pub fn eval_f64(&self, expr: &str) -> Result<f64, DslError> {
        let program = self.compile_expr(expr)?;
        let cel_ctx = self.to_cel_context();
        let value = program
            .execute(&cel_ctx)
            .map_err(|e| DslError::Execution(format!("{e:?}")))?;

        match value {
            Value::Float(f) => Ok(f),
            Value::Int(i) => Ok(i as f64),
            Value::UInt(u) => Ok(u as f64),
            other => Err(DslError::TypeMismatch(format!("{other:?}"))),
        }
    }

    /// Evaluates a CEL expression expecting a boolean result.
    pub fn eval_bool(&self, expr: &str) -> Result<bool, DslError> {
        let program = self.compile_expr(expr)?;
        let cel_ctx = self.to_cel_context();
        let value = program
            .execute(&cel_ctx)
            .map_err(|e| DslError::Execution(format!("{e:?}")))?;

        match value {
            Value::Bool(b) => Ok(b),
            other => Err(DslError::TypeMismatch(format!("{other:?}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_or_expr_static() {
        let ctx = GameContext::new(5.0, 100);
        let value = NumberOrExpr::Number(42.0);
        assert!((value.evaluate(&ctx) - 42.0).abs() < f32::EPSILON);
        assert!(!value.is_dynamic());
    }

    #[test]
    fn test_number_or_expr_dynamic() {
        let ctx = GameContext::new(5.0, 100);
        let value = NumberOrExpr::Expr("game.time * 2.0".to_string());
        assert!((value.evaluate(&ctx) - 10.0).abs() < f32::EPSILON);
        assert!(value.is_dynamic());
    }

    #[test]
    fn test_vec2_or_expr_static() {
        let ctx = GameContext::new(0.0, 0);
        let vec = Vec2OrExpr::Static([10.0, 20.0]);
        let result = vec.evaluate(&ctx);
        assert!((result[0] - 10.0).abs() < f32::EPSILON);
        assert!((result[1] - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_game_context_time() {
        let ctx = GameContext::new(3.5, 210);
        let result = ctx.eval_f32("game.time").unwrap();
        assert!((result - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_game_context_frame() {
        let ctx = GameContext::new(3.5, 210);
        let result = ctx.eval_f32("game.frame").unwrap();
        assert!((result - 210.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_cel_expression() {
        let ctx = GameContext::new(10.0, 600);
        let result = ctx.eval_f32("0.2 + 0.1 * game.time").unwrap();
        assert!((result - 1.2).abs() < 0.001);
    }

    #[test]
    fn test_cached_context() {
        let mut ctx = GameContext::with_cache();
        ctx.update(5.0, 300);

        // First evaluation compiles
        let result1 = ctx.eval_f32("game.time * 2.0").unwrap();
        assert!((result1 - 10.0).abs() < f32::EPSILON);

        // Second evaluation uses cache
        ctx.update(10.0, 600);
        let result2 = ctx.eval_f32("game.time * 2.0").unwrap();
        assert!((result2 - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_random_macro_preprocessing() {
        let mut ctx = GameContext::with_cache_and_seed(12345);

        // Test random() preprocessing
        let processed = ctx.preprocess_random("random(0.5, 1.5)");
        let value: f32 = processed.parse().unwrap();
        assert!(value >= 0.5 && value < 1.5);
    }

    #[test]
    fn test_random_macro_deterministic() {
        let mut ctx1 = GameContext::with_cache_and_seed(42);
        let mut ctx2 = GameContext::with_cache_and_seed(42);

        // Same seed should produce same results
        let result1 = ctx1.eval_f32_with_random("random(0.0, 100.0)").unwrap();
        let result2 = ctx2.eval_f32_with_random("random(0.0, 100.0)").unwrap();
        assert!((result1 - result2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_number_or_expr_with_random() {
        let mut ctx = GameContext::with_cache_and_seed(42);

        let value = NumberOrExpr::Expr("random(1.0, 2.0)".to_string());
        let result = value.evaluate_with_random(&mut ctx);
        assert!(result >= 1.0 && result < 2.0);

        // Static number should work unchanged
        let static_value = NumberOrExpr::Number(5.0);
        assert!((static_value.evaluate_with_random(&mut ctx) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_random_in_expression() {
        let mut ctx = GameContext::with_cache_and_seed(12345);

        // random() can be part of a larger expression
        let result = ctx.eval_f32_with_random("random(1.0, 2.0) + 10.0").unwrap();
        assert!(result >= 11.0 && result < 12.0);
    }
}
