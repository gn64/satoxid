//!
//! # Satoxid, a SATisfiability encoding library
//!
//! Satoxid is a library to help with encoding SAT problems with a focus on ergonomics
//! and debugability.
//!
//! ## Example
//! ```rust
//! use satoxid::{CadicalEncoder, constraints::ExactlyK};
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
//! enum Var {
//!     A, B, C
//! }
//!
//! use Var::*;
//!
//! let mut encoder = CadicalEncoder::new();
//!
//! let constraint = ExactlyK {
//!     k: 1,
//!     lits: [A, B, C].iter().copied()
//! };
//!
//! encoder.add_constraint(constraint);
//!
//! if let Some(model) = encoder.solve() {
//!
//!     let true_lits = model.vars()
//!                          .filter(|v| v.is_pos())
//!                          .count();
//!
//!     assert_eq!(true_lits, 1);
//! }
//! ```
//!
//! ## Concepts
//!
//! ### Variables
//! SAT solvers usually use signed integers to represent literals.
//! Depending on the sign, the literal is either positive or negative and the absolute
//! value defines the SAT variable.
//!
//! While this is a simple API for a SAT solver, it can be inconvenient the user
//! to encode a problem like this.
//! Therefore when using Satoxid we do not work directly with integers but define our own
//! variable type where each value of that type is a SAT variable.
//!
//! As an example, say we want to encode the famous puzzle Sudoku
//! (see the examples for a full implementation).
//! We have a 9x9 grid where each tile has a x-y-coordinate and a value.
//! We can represent this like in this struct `Tile`.
//!
//! ```rust
//! #[derive(Debug, Clone, PartialEq, Eq, Hash)]
//! struct Tile {
//!     x: u32,
//!     y: u32,
//!     value: u32,
//! }
//! ```
//! A value of `Tile` has the meaning that the tile at position (`x`, `y`) has the
//! number in `value`.
//!
//! For a type to be usable as a SAT variable it needs to implement [`SatVar`],
//! which just requires the traits [`Debug`], [`Copy`], [`Eq`] and [`Hash`].
//!
//! We now can use `Tile` in constraints but by itself it only represents a positive
//! literal.
//! If we want to use a negative literal we need to wrap it in [`Lit`] which is an enum
//! which cleary defines a positive or negative literal.
//!
//! Finally there is a third type [`VarType`] which can be used as a literal.
//! When using functions like
//! [`add_constraint_implies_repr`](crate::Encoder::add_constraint_implies_repr)
//! Satoxid generates new variables which have no relation to the user defined SAT
//! variable like `Tile`.
//! [`VarType`] enable the user to be able to use such _unnamed_ variables.
//!
//! Internally Satoxid handles the translation from user defined SAT variables to
//! integer SAT variables for the solver using the [`VarMap`] type.
//!
//! A common pattern is to use an enum which lists all possible kinds of variable in the
//! problem.
//! This enum is then used as the main variable type.
//!
//! ### Constraints
//!
//! Satoxid comes with a set of predefined constraints in the [`constraints`] module.
//! A constraint is a type which can be turned into a finite amount of SAT clauses.
//! This is represented using the [`Constraint`] trait.
//!
//! For example if we wanted to constrain our `Tile` type such that every coordinate
//! can only have exactly one value we would use the [`ExactlyK`](crate::constraints::ExactlyK)
//! constraint.
//!
//! ```rust
//! use satoxid::constraints::ExactlyK;
//! # use satoxid::CadicalEncoder;
//! #
//! # #[derive(Debug, Clone, PartialEq, Eq, Hash)]
//! # struct Tile {
//! #     x: u32,
//! #     y: u32,
//! #     value: u32,
//! # }
//! # fn main() {
//! # let mut encoder = CadicalEncoder::new();
//! # let x = 1;
//! # let y = 1;
//!
//! let constraint = ExactlyK {
//!     k: 1,
//!     lits: (1..=9).map(|value| Tile { x, y, value })
//! };
//! encoder.add_constraint(constraint);
//! # }
//! ```
//!
//! For most simple problems the constraints given should suffice, but if necessary the
//! user can create their own by implementing this trait.
//!
//! Sometimes it is necessary to compose multiple different constraints in non trivial
//! ways. (e.g. We want at least four different constraints to be satisfied.)
//! To do so, the [`ConstraintRepr`] trait allows constraints to be encoded to a single
//! new variable which then can be used in other constraints.
//!
//! ### Encoder and Solvers
//!
//! The [`Encoder`] is the main type the user interacts with.
//! It is given the constraints to be encoded and deals with mapping all SAT variables
//! to their corresponding integer SAT variables.
//! Additionally it has a [`debug`](crate::Encoder::debug) flag which enables/disables
//! debug functionality of the backend (printing the encoded constraints somewhere).
//!
//! The clauses generated by constraints are given to a type implementing [`Backend`].
//! Such a backend might be a solver which is able to solve the encoded problem or maybe
//! just prints the clauses somewhere for external use like [`DimacsWriter`].
//!
//! If a backend is capable of solving it implements the [`Solver`] trait and allows the
//! user to call [`solve`](crate::Encoder::solve) on the encoder.
//! By default Satoxid provides the [CaDiCaL](https://github.com/arminbiere/cadical) SAT solver as a backend which can be used
//! with the [`CadicalEncoder`] type definition.
//! This dependency can be disabled using the `cadical` feature.

use core::fmt;
use std::{
    collections::HashSet,
    fmt::Debug,
    hash::Hash,
    ops::{Index, Not},
};

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

    /// This function is used every time a constraint is encoded,
    /// when the `debug` flag of [`Encoder`] is enabled.
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
    fn value(&mut self, var: i32) -> bool;
}

/// Trait used to express a constraint.
/// Constraints generate a finite set of clauses which are passed to the given backend.
pub trait Constraint<V: SatVar>: Debug + Sized + Clone {
    /// Encode `Self` as an constraint using `solver`.
    fn encode<B: Backend>(self, backend: &mut B, varmap: &mut VarMap<V>);
}

/// Trait used to express a constraint which can imply another variable,
/// a so called representative (repr).
///
/// If no repr is supplied (`None`) then the methods have to choose their own repr.
/// It can either be a fresh generated variable using `varmap`, but sometimes the
/// structure of the constraint provides a suitable candidate.
/// The used repr is returned by the methods.
/// If a repr was provided when calling the methods the same repr has to be returned.
// We need this trait because we cannot generally express the implication of a constraint
// to a repr.
// For example if we take all clauses of an AtMostK constraint the input lits
// can (less ore equal k) be correct but unnamed vars can be choosen such that some
// clauses might still be false which then causes repr to be false.
// The behaviour we would want is that repr is false only if the constraint (more than
// k lits are true) is false.
// If a constraint is however able to express this implication it can implement this
// trait.
pub trait ConstraintRepr<V: SatVar>: Constraint<V> {
    /// Encode if `Self` is satisified, that `repr` is true.
    /// Otherwise `repr` is not constrained and can be true or false.
    fn encode_constraint_implies_repr<B: Backend>(
        self,
        repr: Option<i32>,
        backend: &mut B,
        varmap: &mut VarMap<V>,
    ) -> i32;

    /// Encode if and only if `Self` is satisified, that `repr` is true.
    fn encode_constraint_equals_repr<B: Backend>(
        self,
        repr: Option<i32>,
        backend: &mut B,
        varmap: &mut VarMap<V>,
    ) -> i32 {
        let clone = self.clone();

        let repr = self.encode_constraint_implies_repr(repr, backend, varmap);

        util::repr_implies_constraint(clone, repr, backend, varmap);

        repr
    }

    /// Encode that repr is true if the constraint is satisfied.
    /// The implementation can decide if it has the semantics of
    /// [`encode_constraint_implies_repr`](ConstraintRepr::encode_constraint_implies_repr)
    /// or [`encode_constraint_equals_repr`](ConstraintRepr::encode_constraint_equals_repr),
    /// depending on what is cheaper to encode.
    fn encode_constraint_repr_cheap<B: Backend>(
        self,
        repr: Option<i32>,
        backend: &mut B,
        varmap: &mut VarMap<V>,
    ) -> i32 {
        self.encode_constraint_implies_repr(repr, backend, varmap)
    }
}

/// Enum to define the polarity of variables.
/// By itself `Lit` is a constraint, which requires that the variable it wraps is true
/// or false depending on the Variant `Pos` and `Neg`.
///
/// # Example
/// ```rust
/// # use satoxid::{CadicalEncoder, Lit};
/// # fn main() {
/// # let mut encoder = CadicalEncoder::new();
/// encoder.add_constraint(Lit::Pos("a"));
/// encoder.add_constraint(Lit::Neg("b"));
///
/// let model = encoder.solve().unwrap();
/// assert!(model["a"]);
/// assert!(!model["b"]);
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

impl<V: PartialOrd> PartialOrd for Lit<V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        use std::cmp::Ordering::*;

        let o = self.var().partial_cmp(other.var())?;

        if o == Equal {
            match (self, other) {
                (Lit::Pos(_), Lit::Neg(_)) => Less,
                (Lit::Neg(_), Lit::Pos(_)) => Greater,
                (Lit::Pos(_), Lit::Pos(_)) | (Lit::Neg(_), Lit::Neg(_)) => Equal,
            }
        } else {
            o
        }
        .into()
    }
}

impl<V: Ord> Ord for Lit<V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;

        let o = self.var().cmp(other.var());

        if o == Equal {
            match (self, other) {
                (Lit::Pos(_), Lit::Neg(_)) => Less,
                (Lit::Neg(_), Lit::Pos(_)) => Greater,
                (Lit::Pos(_), Lit::Pos(_)) | (Lit::Neg(_), Lit::Neg(_)) => Equal,
            }
        } else {
            o
        }
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

/// Trait which expresses the required trait bounds for a SAT variable.
pub trait SatVar: Debug + Hash + Eq + Clone {}

impl<V: Hash + Eq + Clone + Debug> SatVar for V {}

/// The of successfully solving an encoded problem.
#[derive(Clone)]
pub struct Model<V> {
    assignments: HashSet<VarType<V>>,
}

impl<V: SatVar> Model<V> {
    /// Returns an interator over assigned literals of user defined SAT variables.
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

impl<V, L> Index<L> for Model<V>
where
    V: SatVar,
    L: Into<VarType<V>> + Debug + Clone,
{
    type Output = bool;

    fn index(&self, l: L) -> &Self::Output {
        let lit = l.clone().into();

        if self.assignments.contains(&lit) {
            &true
        } else if self.assignments.contains(&!lit) {
            &false
        } else {
            panic!("Literal {:?} not contained in model!", l);
        }
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

/// Type which represents *every* used SAT variable by the encoder.
/// It is either a _named_ user defined SAT variable.
/// Or an _unnamed_ generated SAT variable.
///
/// Just like [`Lit`] it is a constraint.
///
/// # Example
/// ```rust
/// # use satoxid::{CadicalEncoder, Lit, VarType};
/// # fn main() {
/// # let mut encoder = CadicalEncoder::<&'static str>::new();
/// let named_var = VarType::Named(Lit::Pos("a"));
/// let unnamed_var = VarType::Unnamed(encoder.varmap.new_var());
///
/// encoder.add_constraint(named_var);
/// encoder.add_constraint(unnamed_var);
///
/// let model = encoder.solve().unwrap();
/// assert!(model[Lit::Pos("a")]);
/// assert!(model[unnamed_var]);
/// # }
/// ```
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

impl<V: SatVar> From<Lit<V>> for VarType<V> {
    fn from(l: Lit<V>) -> Self {
        VarType::Named(l)
    }
}

impl<V: SatVar> From<V> for VarType<V> {
    fn from(v: V) -> Self {
        VarType::Named(Lit::Pos(v))
    }
}

/// The Encoder type contains all data used for the encoding.
#[derive(Clone)]
pub struct Encoder<V, S> {
    pub backend: S,
    pub varmap: VarMap<V>,
    pub debug: bool,
}

impl<V: SatVar, S: Default> Encoder<V, S> {
    /// Creates a new encoder.
    pub fn new() -> Self {
        Self {
            backend: S::default(),
            varmap: VarMap::default(),
            debug: false,
        }
    }

    /// Creates a new encoder and will print out every encoded constraint.
    pub fn with_debug() -> Self {
        Self {
            backend: S::default(),
            varmap: VarMap::default(),
            debug: true,
        }
    }
}

impl<V: SatVar, S: Default> Default for Encoder<V, S> {
    fn default() -> Self {
        Self::new()
    }
}

struct DisplayAsDebug<T>(T);

impl<T: fmt::Display> fmt::Debug for DisplayAsDebug<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <T as fmt::Display>::fmt(&self.0, f)
    }
}

impl<V, B> Encoder<V, B>
where
    V: SatVar,
    B: Backend,
{
    /// Create a new Encoder using `backend` as the Backend.
    pub fn with_backend(backend: B) -> Self {
        Self {
            backend,
            varmap: VarMap::default(),
            debug: false,
        }
    }

    /// Encode a constraint.
    pub fn add_constraint<C: Constraint<V>>(&mut self, constraint: C) {
        if self.debug {
            self.backend.add_debug_info(&constraint);
        }
        constraint.encode(&mut self.backend, &mut self.varmap);
    }

    /// Encode a constraint such that a variable represents it.
    /// If the constraint in the solved model is true, the return variable (repr) will
    /// also be true.
    /// Otherwise it doesn't constrain repr which can either be true or false.
    pub fn add_constraint_implies_repr<C: ConstraintRepr<V>>(
        &mut self,
        constraint: C,
    ) -> VarType<V> {
        if self.debug {
            self.backend.add_debug_info(&constraint);
        }

        let repr = constraint.encode_constraint_implies_repr(
            None,
            &mut self.backend,
            &mut self.varmap,
        );

        if self.debug {
            self.backend
                .append_debug_info(DisplayAsDebug(format!(" => {}", repr)));
        }

        VarType::Unnamed(repr)
    }

    /// Encode a constraint such that a variable represents it.
    /// Like `add_constraint_implies_repr` but the value of repr will equal the
    /// constraint satisfied.
    /// So if constraint wasn't satisfied, repr will be false.
    pub fn add_constraint_equals_repr<C: ConstraintRepr<V>>(
        &mut self,
        constraint: C,
    ) -> VarType<V> {
        if self.debug {
            self.backend.add_debug_info(&constraint);
        }

        let repr = constraint.encode_constraint_equals_repr(
            None,
            &mut self.backend,
            &mut self.varmap,
        );

        if self.debug {
            self.backend
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
        let result = self.backend.solve();

        if result {
            let assignments = self
                .varmap
                .iter_internal_vars()
                .map(|v| {
                    let v = v as i32;
                    let assignment = self.backend.value(v);

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
