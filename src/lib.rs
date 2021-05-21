use core::fmt;
use std::{collections::HashSet, fmt::Debug, hash::Hash, ops::Not};

pub mod constraints;

mod circuit;
mod varmap;

pub use varmap::VarMap;

mod backend;

pub use backend::DimacsWriter;

#[cfg(feature = "cadical")]
pub use backend::CadicalEncoder;

use constraints::util;

/// Backend abstraction trait.
pub trait Backend {
    /// Add raw clause as integer SAT variable.
    /// These are usually determined using `VarMap`.
    fn add_clause<I>(&mut self, lits: I)
    where
        I: Iterator<Item = i32>;

    fn add_debug_info<D: Debug>(&mut self, _debug: D) {}

    fn append_debug_info<D: Debug>(&mut self, _debug: D) {}
}

/// A trait for Backends with are capable of solving SAT Problems.
pub trait Solver: Backend {
    /// Solve the encoded SAT problem.
    /// Returns true if the problem is satisfiable.
    fn solve(&mut self) -> bool;

    /// Returns if the integer SAT variable is true in the model or not.
    ///
    /// This function should panic if solve wasn't called previously or wasn't able to
    /// solve the problem.
    fn value(&self, var: i32) -> bool;
}

/// Trait used to express a constraint.
/// Constraints define a finite set of clauses.
pub trait Constraint<V: SatVar>: Debug + Sized + Clone {
    /// Encode `Self` as an constraint using `solver`.
    fn encode<S: Backend>(self, solver: &mut S, varmap: &mut VarMap<V>);
}

/// Trait used to express a constraint which can imply another variable,
/// a so called representative (repr).
/// If no repr is supplied (`None`) then the methods have to choose their own repr.
/// It can either be newly generated using `varmap`, but sometimes the structure of the
/// constraint provides a suitable candidate.
/// The used repr is returned by the methods.
/// If a repr was provided when calling the methods the same repr has to be returned.
/// If the constraint isn't satisified the whole encoding has to be satisfiable with
/// no matter what value repr is.
// We need this trait because we cannot generally express the implication of a constraint
// to a repr.
// For example if we take all clauses of an AtMostK constraint the input lits
// can (less ore equal k) be correct but unnamed vars can be choosen such that some
// clauses might still be false which then causes repr to be false.
// The behaviour we would want is that repr is false only if the constraint (more than
// k lits are true) is false.
// If a constraint is however able to express this implication it can implement it.
pub trait ConstraintRepr<V: SatVar>: Constraint<V> {
    /// Encode if `Self` is satisified, that `repr` is true.
    fn encode_constraint_implies_repr<S: Backend>(
        self,
        repr: Option<i32>,
        solver: &mut S,
        varmap: &mut VarMap<V>,
    ) -> i32;

    /// Encode if and only if `Self` is satisified, that `repr` is true.
    fn encode_constraint_equals_repr<S: Backend>(
        self,
        repr: Option<i32>,
        solver: &mut S,
        varmap: &mut VarMap<V>,
    ) -> i32 {
        let clone = self.clone();

        let repr = self.encode_constraint_implies_repr(repr, solver, varmap);

        util::repr_implies_constraint(clone, repr, solver, varmap);

        repr
    }

    /// Encode that repr is true if the constraint is satisfied.
    /// The semantics are less restrictive for to allow for cheaper encoding.
    /// No guarantees are given about the constraints of repr if the constraint is false.
    /// Usually it has either the semantics of implies_repr or equals_repr.
    fn encode_constraint_repr_cheap<S: Backend>(
        self,
        repr: Option<i32>,
        solver: &mut S,
        varmap: &mut VarMap<V>,
    ) -> i32 {
        self.encode_constraint_implies_repr(repr, solver, varmap)
    }
}

/// Enum to describe the polarity of variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum Lit<V> {
    Pos(V),
    Neg(V),
}

impl<V> Lit<V> {
    /// Returns the underlying variable.
    pub fn var(&self) -> &V {
        match self {
            Lit::Pos(v) | Lit::Neg(v) => v,
        }
    }

    /// Returns the owned underlying variable
    pub fn unwrap(self) -> V {
        match self {
            Lit::Pos(v) | Lit::Neg(v) => v,
        }
    }

    /// Returns true if `Lit` is positive.
    pub fn is_pos(&self) -> bool {
        matches!(self, Self::Pos(_))
    }

    /// Returns false if `Lit` is negative.
    pub fn is_neg(&self) -> bool {
        matches!(self, Self::Pos(_))
    }
}

impl<V> Not for Lit<V> {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Lit::Pos(v) => Lit::Neg(v),
            Lit::Neg(v) => Lit::Pos(v),
        }
    }
}

/// Trait which expresses the required trait bounds for a variable.
pub trait SatVar: Debug + Hash + Eq + Clone {}

impl<V: Hash + Eq + Clone + Debug> SatVar for V {}

/// Result of solving.
#[derive(Clone)]
pub struct Model<V> {
    assignments: HashSet<VarType<V>>,
}

impl<V: SatVar> Model<V> {
    /// Returns an interator over assigned literals of user defined variables.
    pub fn vars(&self) -> impl Iterator<Item = Lit<V>> + Clone + '_ {
        self.all_vars().filter_map(|v| match v {
            VarType::Named(v) => Some(v),
            VarType::Unnamed(_) => None,
        })
    }

    /// Returns an interator over all defined variables.
    /// This includes unnamed variables used by various constraints.
    pub fn all_vars(&self) -> impl Iterator<Item = VarType<V>> + Clone + '_ {
        self.assignments.iter().cloned()
    }

    /// Returns the assignment of a variable.
    /// Returns `None` if `v` was never used.
    pub fn var(&self, v: V) -> Option<bool> {
        let contains_pos = self
            .assignments
            .contains(&VarType::Named(Lit::Pos(v.clone())));
        let contains_neg = self.assignments.contains(&VarType::Named(Lit::Neg(v)));

        match (contains_pos, contains_neg) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            (false, false) => None,
            (true, true) => unreachable!(),
        }
    }

    /// Returns the assignment of a literal.
    /// Returns `None` if `lit` was never used.
    pub fn lit(&self, lit: Lit<V>) -> Option<bool> {
        let is_pos = lit.is_pos();

        let v = self.var(lit.unwrap())?;

        if is_pos {
            Some(v)
        } else {
            Some(!v)
        }
    }

    #[allow(unused)]
    pub(crate) fn lit_internal(&self, lit: VarType<V>) -> bool {
        self.assignments.contains(&lit)
    }
}

impl<V: SatVar + Ord> Model<V> {
    #[allow(unused)]
    pub(crate) fn print_model(&self) {
        println!("{:?}", {
            let mut m = self.all_vars().collect::<Vec<_>>();
            m.sort();
            m
        });
    }
}

impl<V: SatVar + Ord> Debug for Model<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut model: Vec<_> = self.vars().collect();
        model.sort();

        model.fmt(f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VarType<V> {
    Named(Lit<V>),
    Unnamed(i32),
}

impl<V: fmt::Debug> fmt::Debug for VarType<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VarType::Named(v) => v.fmt(f),
            VarType::Unnamed(v) => f.debug_tuple("Unnamed").field(v).finish(),
        }
    }
}

impl<V> Not for VarType<V> {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            VarType::Named(v) => VarType::Named(!v),
            VarType::Unnamed(v) => VarType::Unnamed(-v),
        }
    }
}

impl<V> From<Lit<V>> for VarType<V> {
    fn from(l: Lit<V>) -> Self {
        VarType::Named(l)
    }
}

/// Encoder abstraction.
pub struct Encoder<V, S> {
    pub solver: S,
    pub varmap: VarMap<V>,
    pub debug: bool,
}

impl<V: SatVar, S: Default> Encoder<V, S> {
    /// Creates a new encoder.
    pub fn new() -> Self {
        Self {
            solver: S::default(),
            varmap: VarMap::default(),
            debug: false,
        }
    }

    /// Creates a new encoder and will print out every encoded constraint.
    pub fn with_debug() -> Self {
        Self {
            solver: S::default(),
            varmap: VarMap::default(),
            debug: true,
        }
    }
}

struct DisplayAsDebug<T>(T);

impl<T: fmt::Display> fmt::Debug for DisplayAsDebug<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <T as fmt::Display>::fmt(&self.0, f)
    }
}

impl<V, S> Encoder<V, S>
where
    V: SatVar,
    S: Backend,
{
    pub fn with_backend(s: S) -> Self {
        Self {
            solver: s,
            varmap: VarMap::default(),
            debug: false,
        }
    }

    /// Encode a constraint.
    pub fn add_constraint<C: Constraint<V>>(&mut self, constraint: C) {
        if self.debug {
            self.solver.add_debug_info(&constraint);
        }
        constraint.encode(&mut self.solver, &mut self.varmap);
    }

    pub fn add_constraint_implies_repr<C: ConstraintRepr<V>>(
        &mut self,
        constraint: C,
    ) -> VarType<V> {
        if self.debug {
            self.solver.add_debug_info(&constraint);
        }

        let repr = constraint.encode_constraint_implies_repr(
            None,
            &mut self.solver,
            &mut self.varmap,
        );

        if self.debug {
            self.solver
                .append_debug_info(DisplayAsDebug(format!(" => {}", repr)));
        }

        VarType::Unnamed(repr)
    }

    pub fn add_constraint_equals_repr<C: ConstraintRepr<V>>(
        &mut self,
        constraint: C,
    ) -> VarType<V> {
        if self.debug {
            self.solver.add_debug_info(&constraint);
        }

        let repr = constraint.encode_constraint_equals_repr(
            None,
            &mut self.solver,
            &mut self.varmap,
        );

        if self.debug {
            self.solver
                .append_debug_info(DisplayAsDebug(format!(" == {}", repr)));
        }

        VarType::Unnamed(repr)
    }
}

impl<V: SatVar, S: Solver> Encoder<V, S> {
    /// Solve the encoded problem.
    /// If problem is unsat then `None` is returned.
    /// Otherwise a model of the problem is returned.
    pub fn solve(&mut self) -> Option<Model<V>> {
        let result = self.solver.solve();

        if result {
            let assignments = self
                .varmap
                .iter_internal_vars()
                .map(|v| {
                    let v = v as i32;
                    let assignment = self.solver.value(v);

                    if let Some(var) = self.varmap.lookup(v) {
                        let var = var.unwrap();
                        let lit = if assignment {
                            Lit::Pos(var)
                        } else {
                            Lit::Neg(var)
                        };
                        VarType::Named(lit)
                    } else {
                        let lit = if assignment { v } else { -v };
                        VarType::Unnamed(lit)
                    }
                })
                .collect();
            Some(Model { assignments })
        } else {
            None
        }
    }
}
