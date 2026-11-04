//! IEEE 1800-2023 §11.2.1, §23.10: a constant expression folded in a given
//! scope, and nothing else.
//!
//! `const_eval_i64_with_params` answers for names its table lacks: from the
//! package and `$unit` snapshot of the last elaboration (which holds the top
//! module's parameters), from struct layouts registered by bare name, and a
//! type it cannot find is 32 bits. Elaboration has to produce some value, so
//! that suits it. A consumer asking what one definition's scope says, such as
//! a check deciding an error, needs to know when the scope does not say.
//!
//! These fold only when every name the expression reads is an entry of the
//! table, under a key the fold looks up, and every operator is one it folds.
//! Anything else is `None`.

use super::{const_eval_i64_with_params, const_index_base_keys, const_parameter_path};
use super::{lookup_scoped_type, resolve_type_width};
use crate::ast::expr::{ExprKind, Expression};
use crate::ast::types::{DataType, PackedDimension};
use crate::hasher::HashMap;
use crate::value::Value;

/// The system functions the fold evaluates from their arguments alone.
const PURE_SYSTEM_FUNCTIONS: &[&str] = &[
    "$clog2",
    "$signed",
    "$unsigned",
    "$countones",
    "$onehot",
    "$onehot0",
];

/// `expr` folded against `params` alone.
pub fn fold_in_scope(expr: &Expression, params: &HashMap<String, Value>) -> Option<i64> {
    if !reads_only(expr, params) {
        return None;
    }
    const_eval_i64_with_params(expr, Some(params))
}

/// The width of an integral `dt` whose dimensions fold against `params` and
/// whose type names `typedefs` holds.
pub fn type_width_in_scope(
    dt: &DataType,
    params: &HashMap<String, Value>,
    typedefs: &HashMap<String, u32>,
) -> Option<u32> {
    let dims_fold = |dims: &[PackedDimension]| {
        dims.iter().all(|d| match d {
            PackedDimension::Range { left, right, .. } => {
                reads_only(left, params) && reads_only(right, params)
            }
            PackedDimension::Unsized(_) => false,
        })
    };
    let known = match dt {
        DataType::IntegerAtom { .. } => true,
        DataType::IntegerVector { dimensions, .. } | DataType::Implicit { dimensions, .. } => {
            dims_fold(dimensions)
        }
        DataType::TypeReference {
            name,
            dimensions,
            type_args,
            ..
        } => {
            type_args.is_empty()
                && lookup_scoped_type(name, typedefs).is_some_and(|w| *w != 0)
                && dims_fold(dimensions)
        }
        _ => false,
    };
    let width = resolve_type_width(dt, Some(params), Some(typedefs));
    (known && width != 0).then_some(width)
}

fn reads_only(expr: &Expression, params: &HashMap<String, Value>) -> bool {
    match &expr.kind {
        ExprKind::Number(_) => true,
        ExprKind::Paren(inner) => reads_only(inner, params),
        ExprKind::Unary { operand, .. } => reads_only(operand, params),
        ExprKind::Binary { left, right, .. } => {
            reads_only(left, params) && reads_only(right, params)
        }
        ExprKind::Conditional {
            condition,
            then_expr,
            else_expr,
        } => {
            reads_only(condition, params)
                && reads_only(then_expr, params)
                && reads_only(else_expr, params)
        }
        ExprKind::SystemCall { name, args } => {
            PURE_SYSTEM_FUNCTIONS.contains(&name.as_str())
                && args.iter().all(|a| reads_only(a, params))
        }
        ExprKind::Ident(h) => {
            if h.root.is_some() {
                return false;
            }
            let selects_fold = h
                .path
                .iter()
                .flat_map(|s| &s.selects)
                .all(|s| reads_only(s, params));
            // A two-segment name is `pkg::x` or `inst.x`; the fold tries both.
            let scoped = match h.path.as_slice() {
                [.., pkg, leaf] if pkg.selects.is_empty() && leaf.selects.is_empty() => {
                    params.contains_key(&format!("{}::{}", pkg.name.name, leaf.name.name))
                }
                _ => false,
            };
            selects_fold && (scoped || has_entry(expr, params))
        }
        // A member of a struct or interface-port parameter has an entry of its
        // own; the fold finds others by layouts this scope may not own.
        ExprKind::MemberAccess { expr: base, member } => {
            let scoped = matches!(&base.kind, ExprKind::Ident(h)
                if h.root.is_none() && h.path.len() == 1 && h.path[0].selects.is_empty()
                    && params.contains_key(&format!("{}::{}", h.path[0].name.name, member.name)));
            scoped || has_entry(expr, params)
        }
        // An element of an array parameter, or a select of a packed one.
        ExprKind::Index { expr: base, index } => {
            if !reads_only(index, params) {
                return false;
            }
            let Some(at) = const_eval_i64_with_params(index, Some(params)) else {
                return false;
            };
            let mut keys = Vec::new();
            const_index_base_keys(base, Some(params), &mut keys);
            keys.iter().any(|k| params.contains_key(&format!("{k}[{at}]")))
                || reads_only(base, params)
        }
        _ => false,
    }
}

fn has_entry(expr: &Expression, params: &HashMap<String, Value>) -> bool {
    const_parameter_path(expr, Some(params)).is_some_and(|k| params.contains_key(&k))
}
