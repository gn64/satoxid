use std::fmt;

use crate::{Backend, Encoder, SolveResult, Solver};

/// Encoder using the CaDiCal SAT solver.
pub type CadicalEncoder<V> = Encoder<V, cadical::Solver>;

impl Backend for cadical::Solver {
    fn add_clause<I>(&mut self, lits: I)
    where
        I: Iterator<Item = i32>,
    {
        self.add_clause(lits.into_iter());
    }

    fn add_debug_info<D: fmt::Debug>(&mut self, debug: D) {
        println!("{:#?}", debug)
    }

    fn append_debug_info<D: fmt::Debug>(&mut self, debug: D) {
        println!("{:?}", debug)
    }
}

impl Solver for cadical::Solver {
    fn solve(&mut self) -> SolveResult {
        match self.solve() {
            Some(true) => SolveResult::Sat,
            Some(false) => SolveResult::Unsat(None),
            None => SolveResult::Unknown,
        }
    }

    fn value(&mut self, var: i32) -> bool {
        self.value(var).unwrap_or(true)
    }
}
