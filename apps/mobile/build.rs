#[path = "../../scripts/build/docs_vault.rs"]
mod docs_vault;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    docs_vault::bundle_docs()
}
