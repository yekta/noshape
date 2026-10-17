//! Feature evaluators, dispatched by feature type id.

use super::eval::{EvalCtx, FResult};

pub mod basic;
pub mod detail;
pub mod extrude;
pub mod revolve;
pub mod sketch;

pub type EvalFn = fn(&mut EvalCtx) -> FResult<()>;

pub fn evaluator(type_id: &str) -> Option<EvalFn> {
    Some(match type_id {
        "sketch" => sketch::eval,
        "extrude" => extrude::eval,
        "revolve" => revolve::eval,
        "fillet" => detail::fillet,
        "chamfer" => detail::chamfer,
        "shell" => detail::shell,
        "booleanBodies" => basic::boolean,
        "cPlane" => basic::plane,
        "assignVariable" => basic::variable,
        "deleteBodies" => basic::delete_bodies,
        _ => return None,
    })
}
