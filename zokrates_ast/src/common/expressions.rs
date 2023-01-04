use num_bigint::BigUint;

use super::operators::OpEq;
use super::{operators::OperatorStr, Span, Value, WithSpan};
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone, PartialEq, Debug, Hash, Eq, PartialOrd, Ord)]
pub struct BinaryExpression<Op, L, R, Out> {
    pub span: Option<Span>,
    pub left: Box<L>,
    pub right: Box<R>,
    operator: PhantomData<Op>,
    output: PhantomData<Out>,
}

impl<Op, L, R, Out> BinaryExpression<Op, L, R, Out> {
    pub fn new(left: L, right: R) -> Self {
        Self {
            span: None,
            left: box left,
            right: box right,
            operator: PhantomData,
            output: PhantomData,
        }
    }
}

impl<Op: OperatorStr, L: fmt::Display, R: fmt::Display, Out: fmt::Display> fmt::Display
    for BinaryExpression<Op, L, R, Out>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({} {} {} /* {} */)",
            self.left,
            Op::STR,
            self.right,
            self.span
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or("".to_string())
        )
    }
}

impl<Op, L, R, Out> WithSpan for BinaryExpression<Op, L, R, Out> {
    fn span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    fn get_span(&self) -> Option<Span> {
        self.span
    }
}

pub enum BinaryOrExpression<Op, L, R, E, I> {
    Binary(BinaryExpression<Op, L, R, E>),
    Expression(I),
}

pub type EqExpression<E, B> = BinaryExpression<OpEq, E, E, B>;

#[derive(Clone, PartialEq, Debug, Hash, Eq, PartialOrd, Ord)]
pub struct UnaryExpression<Op, In, Out> {
    pub span: Option<Span>,
    pub inner: Box<In>,
    operator: PhantomData<Op>,
    output: PhantomData<Out>,
}

impl<Op, In, Out> UnaryExpression<Op, In, Out> {
    pub fn new(inner: In) -> Self {
        Self {
            span: None,
            inner: box inner,
            operator: PhantomData,
            output: PhantomData,
        }
    }
}

impl<Op: OperatorStr, In: fmt::Display, Out: fmt::Display> fmt::Display
    for UnaryExpression<Op, In, Out>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}{})", Op::STR, self.inner,)
    }
}

impl<Op, In, Out> WithSpan for UnaryExpression<Op, In, Out> {
    fn span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    fn get_span(&self) -> Option<Span> {
        self.span
    }
}

pub enum UnaryOrExpression<Op, In, E, I> {
    Unary(UnaryExpression<Op, In, E>),
    Expression(I),
}

#[derive(Clone, PartialEq, Debug, Hash, Eq, PartialOrd, Ord)]
pub struct ValueExpression<V> {
    pub span: Option<Span>,
    pub value: V,
}

impl<V> ValueExpression<V> {
    pub fn new(value: V) -> Self {
        Self { span: None, value }
    }
}

impl<V: fmt::Display> fmt::Display for ValueExpression<V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} /* {} */",
            self.value,
            self.span
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or("".to_string())
        )
    }
}

pub type FieldValueExpression<T> = ValueExpression<T>;

pub type BooleanValueExpression = ValueExpression<bool>;

pub type UValueExpression = ValueExpression<u128>;

pub type IntValueExpression = ValueExpression<BigUint>;

pub enum ValueOrExpression<V, I> {
    Value(V),
    Expression(I),
}

#[derive(Clone, PartialEq, Debug, Hash, Eq, PartialOrd, Ord)]
pub struct IdentifierExpression<I, E> {
    pub span: Option<Span>,
    pub id: I,
    pub ty: PhantomData<E>,
}

impl<I, E> IdentifierExpression<I, E> {
    pub fn new(id: I) -> Self {
        IdentifierExpression {
            span: None,
            id,
            ty: PhantomData,
        }
    }
}

impl<I: fmt::Display, E> fmt::Display for IdentifierExpression<I, E> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl<I, T> WithSpan for IdentifierExpression<I, T> {
    fn span(mut self, span: Option<Span>) -> Self {
        self.span = span;
        self
    }

    fn get_span(&self) -> Option<Span> {
        self.span
    }
}

pub enum IdentifierOrExpression<V, E, I> {
    Identifier(IdentifierExpression<V, E>),
    Expression(I),
}
