use tantivy::Term;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query};
use tantivy::schema::Field;

#[must_use]
pub fn build_query(query_str: &str, name_field: Field, content_field: Field) -> Box<dyn Query> {
    let subqueries: Vec<_> = query_str
        .split_whitespace()
        .map(|word| {
            let word_lower = word.to_lowercase();
            let name_term = Term::from_field_text(name_field, &word_lower);
            let content_term = Term::from_field_text(content_field, &word_lower);
            let name_clause = (
                Occur::Should,
                boxed(FuzzyTermQuery::new_prefix(name_term, 2, true)),
            );
            let content_clause = (
                Occur::Should,
                boxed(FuzzyTermQuery::new_prefix(content_term, 2, true)),
            );
            (
                Occur::Must,
                boxed(BooleanQuery::new(vec![name_clause, content_clause])),
            )
        })
        .collect();

    Box::new(BooleanQuery::new(subqueries))
}

fn boxed<Q: Query + 'static>(query: Q) -> Box<dyn Query> {
    Box::new(query)
}
