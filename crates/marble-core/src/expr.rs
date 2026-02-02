use std::{cell::RefCell, collections::HashMap, sync::Arc};

use cel_interpreter::{Context, Program};

#[allow(unused_imports)]
use rapier2d::prelude::*;

use crate::util::{convert_f32, convert_vec_f32};

pub struct ExecutorCache {
    prepared: RefCell<HashMap<String, Arc<Program>>>,
}

#[derive(Debug)]
pub enum ScalarExpr {
    Constant(f32),
    Cel(String),
}

#[derive(Debug)]
pub enum VectorExpr {
    Constant(Vec<f32>),
    Cel(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ExprError {
    #[error("Expression did not evaluate to the expected type: {0}")]
    ResultTypeMismatch(&'static str),

    #[error("Unprepared expression: '{0}'")]
    UnpreparedExpression(String),

    #[error(transparent)]
    CelExecutionError(#[from] cel_interpreter::ExecutionError),

    #[error(transparent)]
    CelSyntaxError(#[from] cel_interpreter::ParseErrors),
}

impl ExecutorCache {
    pub fn new() -> Self {
        Self {
            prepared: RefCell::new(HashMap::new()),
        }
    }

    pub fn evaluate_scalar<'b>(
        &self,
        ctx: &'b Context<'b>,
        expr: &ScalarExpr,
    ) -> Result<f32, ExprError> {
        match expr {
            ScalarExpr::Constant(v) => Ok(*v),
            ScalarExpr::Cel(s) => {
                let program = self.prepared.borrow().get(s).map(Clone::clone);
                let program = match program {
                    Some(program) => program,
                    None => {
                        let program = Program::compile(s).map_err(ExprError::from)?;
                        let program = Arc::new(program);
                        self.prepared
                            .borrow_mut()
                            .insert(s.clone(), program.clone());
                        program
                    }
                };
                let result = program.execute(ctx)?;
                convert_f32(result).map_err(ExprError::from)
            }
        }
    }

    pub fn evaluate_vector<'b>(
        &self,
        ctx: &'b Context<'b>,
        expr: &VectorExpr,
    ) -> Result<Vector, ExprError> {
        match expr {
            VectorExpr::Constant(v) => {
                if v.len() != 2 {
                    return Err(ExprError::ResultTypeMismatch("Vector of length 2"));
                }
                Ok(Vector::new(v[0], v[1]))
            }
            VectorExpr::Cel(s) => {
                let program = self.prepared.borrow().get(s).map(Clone::clone);
                let program = match program {
                    Some(program) => program,
                    None => {
                        let program = Program::compile(s).map_err(ExprError::from)?;
                        let program = Arc::new(program);
                        self.prepared
                            .borrow_mut()
                            .insert(s.clone(), program.clone());
                        program
                    }
                };
                let result = program.execute(ctx)?;
                let vec = convert_vec_f32(result).map_err(ExprError::from)?;
                if vec.len() != 2 {
                    return Err(ExprError::ResultTypeMismatch("Vector of length 2"));
                }
                Ok(Vector::new(vec[0], vec[1]))
            }
        }
    }
}
