use tantivy::Term;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query};
use tantivy::schema::Field;

pub fn build_query(query_str: &str, name_field: Field, content_field: Field) -> Box<dyn Query> {
    let subqueries: Vec<_> = query_str
        .split_whitespace()
        .map(|word| {
            let word_lower = word.to_lowercase();
            let name_term = Term::from_field_text(name_field, &word_lower);
            let content_term = Term::from_field_text(content_field, &word_lower);
            let name_clause = (
                Occur::Should,
                Box::new(FuzzyTermQuery::new_prefix(name_term, 2, true)) as Box<dyn Query>,
            );
            let content_clause = (
                Occur::Should,
                Box::new(FuzzyTermQuery::new_prefix(content_term, 2, true)) as Box<dyn Query>,
            );
            (
                Occur::Must,
                Box::new(BooleanQuery::new(vec![name_clause, content_clause])) as Box<dyn Query>,
            )
        })
        .collect();

    Box::new(BooleanQuery::new(subqueries))
}
