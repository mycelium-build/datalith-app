use std::collections::BTreeSet;
use std::sync::OnceLock;

use levenshtein_automata::{DFA, Distance, LevenshteinAutomatonBuilder, SINK_STATE};
use tantivy::query::{BooleanQuery, BoostQuery, EmptyQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption};
use tantivy::{Searcher, Term};
use tantivy_fst::Automaton;

/// Matches in file names weigh more than matches in the body.
const NAME_FIELD_BOOST: f32 = 3.0;
/// Guards against pathological short prefixes expanding to every term.
const MAX_PREFIX_EXPANSIONS: usize = 256;
/// Maximum edit distance tolerated in the prefix, as in the previous fuzzy query.
const MAX_EDIT_DISTANCE: u8 = 2;

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

        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for term in expand_prefix_terms(searcher, name_field, &word) {
            let term_query = TermQuery::new(term, IndexRecordOption::WithFreqs);
            clauses.push((
                Occur::Should,
                Box::new(BoostQuery::new(Box::new(term_query), NAME_FIELD_BOOST)),
            ));
        }
        for term in expand_prefix_terms(searcher, content_field, &word) {
            clauses.push((
                Occur::Should,
                Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
            ));
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

/// Expands a word into every indexed term that matches it as a prefix,
/// tolerating up to [`MAX_EDIT_DISTANCE`] edits in that prefix.
///
/// `TermQuery` scores each expanded term with BM25,
/// so term frequency and document length still contribute.
fn expand_prefix_terms(searcher: &Searcher, field: Field, prefix: &str) -> Vec<Term> {
    let dfa = levenshtein_builder().build_prefix_dfa(prefix);
    let mut terms = BTreeSet::new();

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
            if terms.len() >= MAX_PREFIX_EXPANSIONS {
                break;
            }
            if let Ok(term) = std::str::from_utf8(stream.key()) {
                terms.insert(term.to_string());
            }
        }
        if terms.len() >= MAX_PREFIX_EXPANSIONS {
            break;
        }
    }

    terms
        .into_iter()
        .map(|term| Term::from_field_text(field, &term))
        .collect()
}

fn levenshtein_builder() -> &'static LevenshteinAutomatonBuilder {
    static BUILDER: OnceLock<LevenshteinAutomatonBuilder> = OnceLock::new();
    BUILDER.get_or_init(|| LevenshteinAutomatonBuilder::new(MAX_EDIT_DISTANCE, true))
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
}
