use std::collections::BTreeMap;
use std::sync::OnceLock;

use levenshtein_automata::{DFA, Distance, LevenshteinAutomatonBuilder, SINK_STATE};
use tantivy::query::{BooleanQuery, BoostQuery, EmptyQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption};
use tantivy::{Searcher, Term};
use tantivy_fst::Automaton;

/// Guards against pathological prefixes expanding to every term.
const MAX_PREFIX_EXPANSIONS: usize = 256;
const MAX_EDIT_DISTANCE: u8 = 2;
const NAME_FIELD_BOOST: f32 = 3.0; // NAME_FIELD_BOOST * X_MATCH_BOOST
const EXACT_MATCH_BOOST: f32 = 2.0;
const PREFIX_MATCH_BOOST: f32 = 1.0;
const ONE_EDIT_BOOST: f32 = 0.5;
const TWO_EDITS_BOOST: f32 = 0.25;

#[must_use]
pub fn build_query(
    query_str: &str,
    searcher: &Searcher,
    name_field: Field,
    content_field: Field,
) -> Box<dyn Query> {
    let mut word_clauses = Vec::new();

    for word in query_str.split_whitespace() {
        let word = word.to_lowercase();
        let max_edits = max_edit_distance(&word);

        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for expansion in expand_prefix_terms(searcher, name_field, &word, max_edits) {
            clauses.push(expansion.clause(name_field, NAME_FIELD_BOOST));
        }
        for expansion in expand_prefix_terms(searcher, content_field, &word, max_edits) {
            clauses.push(expansion.clause(content_field, 1.0));
        }

        if clauses.is_empty() {
            // Every word is required, so one unmatched word means no results.
            return Box::new(EmptyQuery);
        }
        let clause: Box<dyn Query> = Box::new(BooleanQuery::new(clauses));
        word_clauses.push((Occur::Must, clause));
    }

    if word_clauses.is_empty() {
        Box::new(EmptyQuery)
    } else {
        Box::new(BooleanQuery::new(word_clauses))
    }
}

/// A term matched by a word, with the weight the match earned.
struct Expansion {
    term: String,
    boost: f32,
}

impl Expansion {
    fn clause(self, field: Field, field_boost: f32) -> (Occur, Box<dyn Query>) {
        let term_query = TermQuery::new(
            Term::from_field_text(field, &self.term),
            IndexRecordOption::WithFreqs,
        );
        let boost = field_boost * self.boost;
        (
            Occur::Should,
            Box::new(BoostQuery::new(Box::new(term_query), boost)),
        )
    }
}

/// Maximum edits tolerated for `word`.
///
/// Short words match almost any term within two edits,
/// so tolerance grows with the length that gives it meaning.
fn max_edit_distance(word: &str) -> u8 {
    match word.chars().count() {
        0..=2 => 0,
        3..=5 => 1,
        _ => MAX_EDIT_DISTANCE,
    }
}

/// Weight of a matched `term` relative to the searched `word`.
fn match_boost(term: &str, word: &str, edits: u8) -> f32 {
    if term == word {
        EXACT_MATCH_BOOST
    } else {
        match edits {
            0 => PREFIX_MATCH_BOOST,
            1 => ONE_EDIT_BOOST,
            _ => TWO_EDITS_BOOST,
        }
    }
}

/// Expands a word into every indexed term matching it as a prefix,
/// tolerating up to `max_edits` edits in that prefix.
///
/// `TermQuery` scores each expanded term with BM25,
/// so term frequency and document length still contribute,
/// while the edit distance decides how much the expansion weighs:
/// the exact term and true prefixes must outrank typo matches.
fn expand_prefix_terms(
    searcher: &Searcher,
    field: Field,
    word: &str,
    max_edits: u8,
) -> Vec<Expansion> {
    let dfa = levenshtein_builder(max_edits).build_prefix_dfa(word);
    let mut closest: BTreeMap<String, u8> = BTreeMap::new();

    for segment in searcher.segment_readers() {
        let Ok(inverted_index) = segment.inverted_index(field) else {
            continue;
        };
        let Ok(mut stream) = inverted_index
            .terms()
            .search(FuzzyPrefixDfa(&dfa))
            .into_stream()
        else {
            continue;
        };
        while stream.advance() {
            let Ok(term) = std::str::from_utf8(stream.key()) else {
                continue;
            };
            let Some(edits) = prefix_edit_distance(&dfa, term) else {
                continue;
            };
            // The same term can live in several segments; the closest match wins.
            closest
                .entry(term.to_string())
                .and_modify(|known| *known = (*known).min(edits))
                .or_insert(edits);
        }
    }

    let mut expansions: Vec<Expansion> = closest
        .into_iter()
        .map(|(term, edits)| Expansion {
            boost: match_boost(&term, word, edits),
            term,
        })
        .collect();
    // The cap must drop typo matches before exact and prefix ones.
    expansions.sort_by(|left, right| {
        right
            .boost
            .total_cmp(&left.boost)
            .then_with(|| left.term.cmp(&right.term))
    });
    expansions.truncate(MAX_PREFIX_EXPANSIONS);
    expansions
}

/// Edits the prefix automaton needed to match `term`, if it matched at all.
fn prefix_edit_distance(dfa: &DFA, term: &str) -> Option<u8> {
    match dfa.eval(term) {
        Distance::Exact(edits) => Some(edits),
        Distance::AtLeast(_) => None,
    }
}

fn levenshtein_builder(max_edits: u8) -> &'static LevenshteinAutomatonBuilder {
    static DISTANCE_0: OnceLock<LevenshteinAutomatonBuilder> = OnceLock::new();
    static DISTANCE_1: OnceLock<LevenshteinAutomatonBuilder> = OnceLock::new();
    static DISTANCE_2: OnceLock<LevenshteinAutomatonBuilder> = OnceLock::new();
    let (slot, distance) = match max_edits {
        0 => (&DISTANCE_0, 0),
        1 => (&DISTANCE_1, 1),
        _ => (&DISTANCE_2, MAX_EDIT_DISTANCE),
    };
    slot.get_or_init(|| LevenshteinAutomatonBuilder::new(distance, true))
}

/// Adapts a `levenshtein_automata` DFA to the automaton trait used by Tantivy's term dictionary.
struct FuzzyPrefixDfa<'a>(&'a DFA);

impl Automaton for FuzzyPrefixDfa<'_> {
    type State = u32;

    fn start(&self) -> Self::State {
        self.0.initial_state()
    }

    fn is_match(&self, state: &Self::State) -> bool {
        matches!(self.0.distance(*state), Distance::Exact(_))
    }

    fn can_match(&self, state: &Self::State) -> bool {
        *state != SINK_STATE
    }

    fn accept(&self, state: &Self::State, byte: u8) -> Self::State {
        self.0.transition(*state, byte)
    }
}

#[cfg(test)]
mod tests {
    use super::build_query;
    use tantivy::collector::TopDocs;
    use tantivy::schema::{STORED, Schema, TEXT};
    use tantivy::{Index, doc};

    fn scored_docs(documents: &[(&str, &str)], query: &str) -> Vec<(u32, f32)> {
        let mut schema_builder = Schema::builder();
        let name = schema_builder.add_text_field("name", TEXT | STORED);
        let content = schema_builder.add_text_field("content", TEXT | STORED);
        let index = Index::create_in_ram(schema_builder.build());
        let mut writer = index.writer_with_num_threads(1, 15_000_000).unwrap();
        for (doc_name, doc_content) in documents {
            writer
                .add_document(doc!(name => *doc_name, content => *doc_content))
                .unwrap();
        }
        writer.commit().unwrap();

        let reader = index.reader().unwrap();
        let searcher = reader.searcher();
        let query = build_query(query, &searcher, name, content);
        searcher
            .search(&query, &TopDocs::with_limit(10).order_by_score())
            .unwrap()
            .into_iter()
            .map(|(score, address)| (address.doc_id, score))
            .collect()
    }

    #[test]
    fn term_frequency_increases_the_score() {
        let results = scored_docs(
            &[
                ("alpha", "needle needle needle filler filler filler"),
                ("beta", "needle filler filler filler filler filler"),
            ],
            "needle",
        );
        let first = results.first().unwrap();
        let second = results.get(1).unwrap();
        assert_eq!(first.0, 0);
        assert!(first.1 > second.1);
    }

    #[test]
    fn file_name_matches_are_boosted() {
        let results = scored_docs(
            &[
                ("needle", "filler filler filler filler filler filler"),
                ("beta", "needle filler filler filler filler filler"),
            ],
            "needle",
        );
        let first = results.first().unwrap();
        let second = results.get(1).unwrap();
        assert_eq!(first.0, 0);
        assert!(first.1 > second.1);
    }

    #[test]
    fn prefix_terms_are_scored() {
        let results = scored_docs(&[("alpha", "datalith is here")], "dat");
        let first = results.first().unwrap();
        assert_eq!(first.0, 0);
        assert!(first.1 > 0.0);
    }

    #[test]
    fn typos_in_the_prefix_are_matched() {
        let results = scored_docs(&[("alpha", "the database is here")], "databse");
        let first = results.first().unwrap();
        assert_eq!(first.0, 0);
        assert!(first.1 > 0.0);
    }

    #[test]
    fn exact_and_prefix_matches_outrank_typo_matches() {
        let results = scored_docs(
            &[("alpha", "dat"), ("beta", "database"), ("gamma", "dxta")],
            "dat",
        );
        let order: Vec<u32> = results.iter().map(|(doc_id, _)| *doc_id).collect();
        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn short_words_do_not_tolerate_typos() {
        let results = scored_docs(&[("alpha", "gist")], "go");
        assert!(results.is_empty());
    }
}
