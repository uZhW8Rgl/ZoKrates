//! Module containing structs and enums to represent a program.
//!
//! @file absy.rs
//! @author Dennis Kuhnert <dennis.kuhnert@campus.tu-berlin.de>
//! @author Jacob Eberhardt <jacob.eberhardt@tu-berlin.de>
//! @date 2017

pub mod folder;
pub mod utils;

use crate::common;
use crate::common::FormatString;
pub use crate::common::Parameter;
pub use crate::common::RuntimeError;
pub use crate::common::Variable;
use crate::common::{
    expressions::{BinaryExpression, IdentifierExpression, ValueExpression},
    operators::*,
};
use crate::common::{Span, WithSpan};

pub use utils::{
    flat_expression_from_bits, flat_expression_from_expression_summands,
    flat_expression_from_variable_summands,
};

use crate::common::Solver;
use crate::typed::ConcreteType;
use std::collections::HashMap;
use std::fmt;
use zokrates_field::Field;

pub type FlatProg<T> = FlatFunction<T>;

pub type FlatFunction<T> = FlatFunctionIterator<T, Vec<FlatStatement<T>>>;

pub type FlatProgIterator<T, I> = FlatFunctionIterator<T, I>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FlatFunctionIterator<T, I: IntoIterator<Item = FlatStatement<T>>> {
    /// Arguments of the function
    pub arguments: Vec<Parameter>,
    /// Vector of statements that are executed when running the function
    pub statements: I,
    /// Number of outputs
    pub return_count: usize,
}

impl<T, I: IntoIterator<Item = FlatStatement<T>>> FlatFunctionIterator<T, I> {
    pub fn collect(self) -> FlatFunction<T> {
        FlatFunction {
            statements: self.statements.into_iter().collect(),
            arguments: self.arguments,
            return_count: self.return_count,
        }
    }
}

impl<T: Field> fmt::Display for FlatFunction<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "def main({}) -> {}:\n{}",
            self.arguments
                .iter()
                .map(|x| format!("{}", x))
                .collect::<Vec<_>>()
                .join(","),
            self.return_count,
            self.statements
                .iter()
                .map(|x| format!("\t{}", x))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

pub type DefinitionStatement<T> =
    common::expressions::DefinitionStatement<Variable, FlatExpression<T>>;
pub type AssertionStatement<T> =
    common::expressions::AssertionStatement<FlatExpression<T>, RuntimeError>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum FlatStatement<T> {
    Condition(AssertionStatement<T>),
    Definition(DefinitionStatement<T>),
    Directive(FlatDirective<T>),
    Log(FormatString, Vec<(ConcreteType, Vec<FlatExpression<T>>)>),
}

impl<T> FlatStatement<T> {
    pub fn definition(assignee: Variable, rhs: FlatExpression<T>) -> Self {
        Self::Definition(DefinitionStatement::new(assignee, rhs))
    }

    pub fn assertion(expression: FlatExpression<T>, error: RuntimeError) -> Self {
        Self::Condition(AssertionStatement::new(expression, error))
    }

    pub fn condition(
        left: FlatExpression<T>,
        right: FlatExpression<T>,
        error: RuntimeError,
    ) -> Self {
        Self::assertion(left - right, error)
    }
}

impl<T> WithSpan for FlatStatement<T> {
    fn span(self, span: Option<Span>) -> Self {
        use FlatStatement::*;

        match self {
            Condition(e) => Condition(e.span(span)),
            Definition(e) => Definition(e.span(span)),
            Directive(_) => todo!(),
            Log(_, _) => todo!(),
        }
    }

    fn get_span(&self) -> Option<Span> {
        use FlatStatement::*;

        match self {
            Condition(e) => e.get_span(),
            Definition(e) => e.get_span(),
            Directive(_) => todo!(),
            Log(_, _) => todo!(),
        }
    }
}

impl<T: Field> fmt::Display for FlatStatement<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FlatStatement::Definition(ref e) => write!(f, "{}", e),
            FlatStatement::Condition(ref s) => {
                write!(f, "{} == 0 // {}", s.expression, s.error)
            }
            FlatStatement::Directive(ref d) => write!(f, "{}", d),
            FlatStatement::Log(ref l, ref expressions) => write!(
                f,
                "log(\"{}\"), {})",
                l,
                expressions
                    .iter()
                    .map(|(_, e)| format!(
                        "[{}]",
                        e.iter()
                            .map(|e| e.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}

impl<T: Field> FlatStatement<T> {
    pub fn apply_substitution(
        self,
        substitution: &HashMap<Variable, Variable>,
    ) -> FlatStatement<T> {
        match self {
            FlatStatement::Definition(s) => FlatStatement::definition(
                *s.assignee.apply_substitution(substitution),
                s.rhs.apply_substitution(substitution),
            ),
            FlatStatement::Condition(s) => {
                FlatStatement::assertion(s.expression.apply_substitution(substitution), s.error)
            }
            FlatStatement::Directive(d) => {
                let outputs = d
                    .outputs
                    .into_iter()
                    .map(|o| *o.apply_substitution(substitution))
                    .collect();
                let inputs = d
                    .inputs
                    .into_iter()
                    .map(|i| i.apply_substitution(substitution))
                    .collect();

                FlatStatement::Directive(FlatDirective {
                    inputs,
                    outputs,
                    ..d
                })
            }
            FlatStatement::Log(l, e) => FlatStatement::Log(
                l,
                e.into_iter()
                    .map(|(t, e)| {
                        (
                            t,
                            e.into_iter()
                                .map(|e| e.apply_substitution(substitution))
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Hash, Debug, PartialEq, Eq)]
pub struct FlatDirective<T> {
    pub inputs: Vec<FlatExpression<T>>,
    pub outputs: Vec<Variable>,
    pub solver: Solver,
}

impl<T> FlatDirective<T> {
    pub fn new<E: Into<FlatExpression<T>>>(
        outputs: Vec<Variable>,
        solver: Solver,
        inputs: Vec<E>,
    ) -> Self {
        let (in_len, out_len) = solver.get_signature();
        assert_eq!(in_len, inputs.len());
        assert_eq!(out_len, outputs.len());
        FlatDirective {
            solver,
            inputs: inputs.into_iter().map(|i| i.into()).collect(),
            outputs,
        }
    }
}

impl<T: Field> fmt::Display for FlatDirective<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "# {} = {}({})",
            self.outputs
                .iter()
                .map(|o| o.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            self.solver,
            self.inputs
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum FlatExpression<T> {
    Number(ValueExpression<T>),
    Identifier(IdentifierExpression<Variable, Self>),
    Add(BinaryExpression<OpAdd, Self, Self, Self>),
    Sub(BinaryExpression<OpSub, Self, Self, Self>),
    Mult(BinaryExpression<OpMul, Self, Self, Self>),
}

impl<T> std::ops::Add for FlatExpression<T> {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        FlatExpression::Add(BinaryExpression::new(self, other))
    }
}

impl<T> std::ops::Sub for FlatExpression<T> {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        FlatExpression::Sub(BinaryExpression::new(self, other))
    }
}

impl<T> std::ops::Mul for FlatExpression<T> {
    type Output = Self;

    fn mul(self, other: Self) -> Self::Output {
        FlatExpression::Mult(BinaryExpression::new(self, other))
    }
}

impl<T> From<T> for FlatExpression<T> {
    fn from(other: T) -> Self {
        Self::number(other)
    }
}

impl<T, Op> BinaryExpression<Op, FlatExpression<T>, FlatExpression<T>, FlatExpression<T>> {
    fn apply_substitution(self, substitution: &HashMap<Variable, Variable>) -> Self {
        let left = self.left.apply_substitution(substitution);
        let right = self.right.apply_substitution(substitution);

        Self::new(left, right).span(self.span)
    }
}

impl<T> IdentifierExpression<Variable, FlatExpression<T>> {
    fn apply_substitution(self, substitution: &HashMap<Variable, Variable>) -> Self {
        let id = *self.id.apply_substitution(substitution);

        IdentifierExpression { id, ..self }
    }
}

impl<T> FlatExpression<T> {
    pub fn identifier(v: Variable) -> Self {
        Self::Identifier(IdentifierExpression::new(v))
    }

    pub fn number(t: T) -> Self {
        Self::Number(ValueExpression::new(t))
    }

    pub fn apply_substitution(self, substitution: &HashMap<Variable, Variable>) -> Self {
        match self {
            e @ FlatExpression::Number(_) => e,
            FlatExpression::Identifier(id) => {
                FlatExpression::Identifier(id.apply_substitution(substitution))
            }
            FlatExpression::Add(e) => FlatExpression::Add(e.apply_substitution(substitution)),
            FlatExpression::Sub(e) => FlatExpression::Sub(e.apply_substitution(substitution)),
            FlatExpression::Mult(e) => FlatExpression::Mult(e.apply_substitution(substitution)),
        }
    }

    pub fn is_linear(&self) -> bool {
        match *self {
            FlatExpression::Number(_) | FlatExpression::Identifier(_) => true,
            FlatExpression::Add(ref e) => e.left.is_linear() && e.right.is_linear(),
            FlatExpression::Sub(ref e) => e.left.is_linear() && e.right.is_linear(),
            FlatExpression::Mult(ref e) => matches!(
                (&e.left, &e.right),
                (box FlatExpression::Number(_), box FlatExpression::Number(_))
                    | (
                        box FlatExpression::Number(_),
                        box FlatExpression::Identifier(_)
                    )
                    | (
                        box FlatExpression::Identifier(_),
                        box FlatExpression::Number(_)
                    )
            ),
        }
    }
}

impl<T: Field> fmt::Display for FlatExpression<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FlatExpression::Number(ref i) => write!(f, "{}", i),
            FlatExpression::Identifier(ref var) => write!(f, "{}", var),
            FlatExpression::Add(ref e) => write!(f, "{}", e),
            FlatExpression::Sub(ref e) => write!(f, "{}", e),
            FlatExpression::Mult(ref e) => write!(f, "{}", e),
        }
    }
}

impl<T: Field> From<Variable> for FlatExpression<T> {
    fn from(v: Variable) -> FlatExpression<T> {
        FlatExpression::identifier(v)
    }
}

#[derive(PartialEq, Eq, Debug)]
pub struct Error {
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl<T> WithSpan for FlatExpression<T> {
    fn span(self, span: Option<Span>) -> Self {
        use FlatExpression::*;
        match self {
            Add(e) => Add(e.span(span)),
            Sub(e) => Sub(e.span(span)),
            Mult(e) => Mult(e.span(span)),
            Number(e) => Number(e.span(span)),
            Identifier(e) => Identifier(e.span(span)),
        }
    }

    fn get_span(&self) -> Option<Span> {
        use FlatExpression::*;
        match self {
            Add(e) => e.get_span(),
            Sub(e) => e.get_span(),
            Mult(e) => e.get_span(),
            Number(e) => e.get_span(),
            Identifier(e) => e.get_span(),
        }
    }
}
