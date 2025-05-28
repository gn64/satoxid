use crate::{Backend, IncrementalSolver, SolveResult, Solver};

#[derive(Default)]
pub struct MockSolver {
    clauses: Vec<Vec<i32>>,
}
impl MockSolver {
    pub fn get_clauses(&self) -> Vec<Vec<i32>> {
        self.clauses.clone()
    }
    pub fn clear_clauses(&mut self) {
        self.clauses.clear();
    }
}

impl Backend for MockSolver {
    fn add_clause<I>(&mut self, lits: I)
    where
        I: Iterator<Item = i32>,
    {
        let clause_i: Vec<i32> = lits.collect();
        self.clauses.push(clause_i);
    }
}

impl Solver for MockSolver {
    fn solve(&mut self) -> SolveResult {
        SolveResult::Unknown
    }

    fn value(&mut self, var: i32) -> bool {
        false
    }
}

impl IncrementalSolver for MockSolver {
    fn assumption_solve<I>(&mut self, assumptions: I) -> SolveResult
    where
        I: Iterator<Item = i32>,
    {
        SolveResult::Unknown
    }
}
