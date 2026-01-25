//! CEL DSL types for dynamic map expressions.
//!
//! Provides types that can be either static values or CEL expressions
//! evaluated at runtime.

use std::collections::HashMap;
use std::sync::Arc;

use cel_interpreter::{Context, Program, Value};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Returns true if this is a dynamic expression (not a static number).
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Expr(_))
    }
}

/// A 2D vector that can be static or dynamic.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Vec2OrExpr {
    /// Static [x, y] values.
    Static([f32; 2]),
    /// Dynamic values where each component can be a number or expression.
    Dynamic([NumberOrExpr; 2]),
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
        }
    }

    /// Creates a game context with expression caching enabled.
    pub fn with_cache() -> Self {
        Self {
            time: 0.0,
            frame: 0,
            program_cache: Some(Arc::new(parking_lot::RwLock::new(HashMap::new()))),
        }
    }

    /// Updates the context with new time and frame values.
    pub fn update(&mut self, time: f32, frame: u64) {
        self.time = time;
        self.frame = frame;
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
}
