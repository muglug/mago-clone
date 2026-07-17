use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::num::Wrapping;

use foldhash::fast::FixedState;
use indexmap::IndexMap;

use mago_codex::assertion::Assertion;
use mago_codex::ttype::TType;
use mago_span::Span;
use mago_word::Word;
use mago_word::WordMap;
use mago_word::WordSet;
use mago_word::concat_word;
use mago_word::empty_word;
use mago_word::word;

#[derive(Clone, Debug, Eq)]
pub struct Clause {
    pub condition_span: Span,
    pub span: Span,
    pub hash: u32,
    pub possibilities: IndexMap<Word, IndexMap<u64, Assertion>>,
    pub wedge: bool,
    pub reconcilable: bool,
    pub generated: bool,
    /// Variables reassigned while evaluating the conditional that produced
    /// this clause. Assertions for these variables describe their
    /// post-assignment values, so they must supersede stale earlier truths.
    pub redefined_vars: WordSet,
    /// The value types written by those assignments. These are kept separate
    /// from the path assertion so negating a condition negates only the test,
    /// not the fact that its assignment has already executed.
    pub redefined_var_types: WordMap<IndexMap<u64, Assertion>>,
}

impl PartialEq for Clause {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Hash for Clause {
    #[inline]
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.hash.hash(state);
    }
}

impl Clause {
    #[inline]
    #[must_use]
    pub fn new(
        possibilities: IndexMap<Word, IndexMap<u64, Assertion>>,
        condition_span: Span,
        span: Span,
        wedge: Option<bool>,
        reconcilable: Option<bool>,
        generated: Option<bool>,
    ) -> Clause {
        let redefined_vars = WordSet::default();
        let redefined_var_types = WordMap::default();
        Clause {
            condition_span,
            span,
            wedge: wedge.unwrap_or(false),
            reconcilable: reconcilable.unwrap_or(true),
            generated: generated.unwrap_or(false),
            hash: get_hash(
                &possibilities,
                &redefined_vars,
                &redefined_var_types,
                span,
                wedge.unwrap_or(false),
                reconcilable.unwrap_or(true),
            ),
            possibilities,
            redefined_vars,
            redefined_var_types,
        }
    }

    /// Marks an assertion in this clause as describing a variable after an
    /// assignment performed by the conditional.
    #[inline]
    #[must_use]
    pub fn mark_redefined(mut self, var_id: Word) -> Clause {
        if self.redefined_vars.insert(var_id) {
            self.refresh_hash();
        }

        self
    }

    /// Marks a post-assignment assertion and records the type produced by the
    /// assignment independently from the condition's truth value.
    #[inline]
    #[must_use]
    pub fn mark_redefined_with_types(mut self, var_id: Word, assigned_types: IndexMap<u64, Assertion>) -> Clause {
        self.redefined_vars.insert(var_id);
        if !assigned_types.is_empty() {
            self.redefined_var_types.insert(var_id, assigned_types);
        }
        self.refresh_hash();
        self
    }

    /// Carries assignment provenance through an algebraic clause rewrite.
    #[inline]
    #[must_use]
    pub fn with_redefined_vars(mut self, redefined_vars: WordSet) -> Clause {
        self.redefined_vars = redefined_vars;
        self.refresh_hash();
        self
    }

    /// Carries the assignment result types through an algebraic clause
    /// rewrite.
    #[inline]
    #[must_use]
    pub fn with_redefined_var_types(mut self, redefined_var_types: WordMap<IndexMap<u64, Assertion>>) -> Clause {
        self.redefined_var_types = redefined_var_types;
        self.refresh_hash();
        self
    }

    #[inline]
    fn refresh_hash(&mut self) {
        self.hash = get_hash(
            &self.possibilities,
            &self.redefined_vars,
            &self.redefined_var_types,
            self.span,
            self.wedge,
            self.reconcilable,
        );
    }

    #[inline]
    #[must_use]
    pub fn remove_possibilities(&self, var_id: Word) -> Option<Clause> {
        let mut possibilities = self.possibilities.clone();

        possibilities.shift_remove(&var_id);

        if possibilities.is_empty() {
            return None;
        }

        Some(
            Clause::new(
                possibilities,
                self.condition_span,
                self.span,
                Some(self.wedge),
                Some(self.reconcilable),
                Some(self.generated),
            )
            .with_redefined_vars(self.redefined_vars.clone())
            .with_redefined_var_types(self.redefined_var_types.clone()),
        )
    }

    #[inline]
    #[must_use]
    pub fn add_possibility(&self, var_id: Word, new_possibility: IndexMap<u64, Assertion>) -> Clause {
        let mut possibilities = self.possibilities.clone();

        possibilities.insert(var_id, new_possibility);

        Clause::new(
            possibilities,
            self.condition_span,
            self.span,
            Some(self.wedge),
            Some(self.reconcilable),
            Some(self.generated),
        )
        .with_redefined_vars(self.redefined_vars.clone())
        .with_redefined_var_types(self.redefined_var_types.clone())
    }

    #[inline]
    #[must_use]
    pub fn contains(&self, other_clause: &Self) -> bool {
        if other_clause.possibilities.len() > self.possibilities.len() {
            return false;
        }

        other_clause.possibilities.iter().all(|(var, possible_types)| {
            self.possibilities
                .get(var)
                .is_some_and(|local_possibilities| possible_types.keys().all(|k| local_possibilities.contains_key(k)))
        })
    }

    #[inline]
    #[must_use]
    pub fn get_impossibilities(&self) -> BTreeMap<Word, Vec<Assertion>> {
        self.possibilities
            .iter()
            .filter_map(|(variable, possibility)| {
                let negations: Vec<Assertion> = possibility.values().map(Assertion::get_negation).collect();

                if negations.is_empty() { None } else { Some((*variable, negations)) }
            })
            .collect()
    }

    #[inline]
    #[must_use]
    pub fn to_atom(&self) -> Word {
        if self.possibilities.is_empty() {
            return word("<empty>");
        }

        let mut final_result = empty_word();
        let mut is_first_clause = true;

        for (var_id, values) in &self.possibilities {
            if !is_first_clause {
                final_result = concat_word!(final_result, " && ");
            }

            is_first_clause = false;

            let var_name = if var_id.as_bytes().starts_with(b"*") { word(b"<expr>") } else { *var_id };

            let mut clause_result = empty_word();
            let mut is_first_part_in_clause = true;
            for (_, value) in values {
                if !is_first_part_in_clause {
                    clause_result = concat_word!(clause_result, " || ");
                }
                is_first_part_in_clause = false;

                let part_atom = match value {
                    Assertion::Any => concat_word!(var_name, " is any"),
                    Assertion::Falsy => concat_word!("!", var_name),
                    Assertion::Truthy => var_name,
                    Assertion::IsType(v) | Assertion::IsIdentical(v) => {
                        concat_word!(var_name, " is ", v.get_id())
                    }
                    Assertion::IsNotType(v) | Assertion::IsNotIdentical(v) => {
                        concat_word!(var_name, " is not ", v.get_id())
                    }
                    _ => value.to_atom(),
                };

                clause_result = concat_word!(clause_result, part_atom);
            }

            if values.len() > 1 {
                clause_result = concat_word!("(", clause_result, ")");
            }

            final_result = concat_word!(final_result, clause_result);
        }

        final_result
    }
}

#[inline]
fn get_hash(
    possibilities: &IndexMap<Word, IndexMap<u64, Assertion>>,
    redefined_vars: &WordSet,
    redefined_var_types: &WordMap<IndexMap<u64, Assertion>>,
    clause_span: Span,
    wedge: bool,
    reconcilable: bool,
) -> u32 {
    if wedge || !reconcilable {
        (Wrapping(clause_span.start.offset)
            + Wrapping(clause_span.end.offset)
            + Wrapping(if wedge { 100_000 } else { 0 }))
        .0
    } else {
        let mut hasher = FixedState::default().build_hasher();
        for possibility in possibilities {
            possibility.0.hash(&mut hasher);
            0.hash(&mut hasher);

            for i in possibility.1.keys() {
                i.hash(&mut hasher);
                1.hash(&mut hasher);
            }
        }

        let mut redefined_vars = redefined_vars.iter().copied().collect::<Vec<_>>();
        redefined_vars.sort_unstable();
        for var_id in redefined_vars {
            var_id.hash(&mut hasher);
            2.hash(&mut hasher);

            if let Some(types) = redefined_var_types.get(&var_id) {
                let mut type_hashes = types.keys().copied().collect::<Vec<_>>();
                type_hashes.sort_unstable();
                for type_hash in type_hashes {
                    type_hash.hash(&mut hasher);
                }
            }
        }

        hasher.finish() as u32
    }
}

impl std::fmt::Display for Clause {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_atom())
    }
}
