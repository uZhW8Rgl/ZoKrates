//! Module containing the `RedefinitionOptimizer` to remove code of the form
// ```
// b := Directive(a)
// c := Directive(a)
// ```
// and replace by
// ```
// b := Directive(a)
// c := b
// ```

use std::collections::hash_map::{Entry, HashMap};
use zokrates_ast::ir::folder::*;
use zokrates_ast::ir::*;
use zokrates_field::Field;

type SolverCall<'ast, T> = (Solver<'ast, T>, Vec<QuadComb<T>>);

#[derive(Debug, Default)]
pub struct DirectiveOptimizer<'ast, T> {
    calls: HashMap<SolverCall<'ast, T>, Vec<Variable>>,
    /// Map of renamings for reassigned variables while processing the program.
    substitution: HashMap<Variable, Variable>,
}

impl<'ast, T: Field> Folder<'ast, T> for DirectiveOptimizer<'ast, T> {
    fn fold_variable(&mut self, v: Variable) -> Variable {
        *self.substitution.get(&v).unwrap_or(&v)
    }

    fn fold_statement(&mut self, s: Statement<'ast, T>) -> Vec<Statement<'ast, T>> {
        match s {
            Statement::Directive(d) => {
                let d = self.fold_directive(d);

                match self.calls.entry((d.solver.clone(), d.inputs.clone())) {
                    Entry::Vacant(e) => {
                        e.insert(d.outputs.clone());
                        vec![Statement::Directive(d)]
                    }
                    Entry::Occupied(e) => {
                        self.substitution
                            .extend(d.outputs.into_iter().zip(e.get().iter().cloned()));
                        vec![]
                    }
                }
            }
            s => fold_statement(self, s),
        }
    }
}

#[cfg(test)]
mod tests {}
