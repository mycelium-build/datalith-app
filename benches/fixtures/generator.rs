use rand::prelude::*;
use rand::rngs::StdRng;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const WORDS: &[&str] = &[
    "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not", "on",
    "with", "he", "as", "you", "do", "at", "this", "but", "his", "by", "from", "they", "we", "say",
    "her", "she", "or", "an", "will", "my", "one", "all", "would", "there", "their", "what", "so",
    "up", "out", "if", "about", "who", "get", "which", "go", "me", "when", "make", "can", "like",
    "time", "no", "just", "him", "know", "take", "people", "into", "year", "your", "good", "some",
    "could", "them", "see", "other", "than", "then", "now", "look", "only", "come", "its", "over",
    "think", "also", "back", "after", "use", "two", "how", "our", "work", "first", "well", "way",
    "even", "new", "want", "because", "any", "these", "give", "day", "most", "us", "great",
    "between", "need", "large", "under", "never", "each", "right", "begin", "always", "those",
    "both", "paper", "together", "often", "run", "small", "open", "might", "still", "keep",
    "start", "point", "read", "hand", "high", "place", "live", "where", "should", "world",
    "school", "through", "every", "change", "move", "play", "found", "study", "learn", "plant",
    "cover", "food", "sun", "four", "thought", "let", "city", "tree", "cross", "farm", "hard",
    "story", "picture", "draw", "left", "late", "while", "press", "close", "night", "real", "life",
    "few", "north", "book", "carry", "took", "science", "eat", "room", "friend", "began", "idea",
    "fish", "mountain", "stop", "once", "base", "hear", "horse", "cut", "sure", "watch", "color",
    "face", "wood", "main", "enough", "plain", "girl", "usual", "young", "ready", "above", "ever",
    "red", "list", "though", "feel", "talk", "bird", "soon", "body", "dog", "family", "direct",
    "pose", "leave", "song", "measure", "door", "product", "black", "short", "numeral", "class",
    "wind", "question", "happen", "complete", "ship", "area", "half", "rock", "order", "fire",
    "south", "problem", "piece", "told", "knew", "pass", "since", "top", "whole", "king", "space",
    "heard", "best", "hour", "better", "true", "during", "hundred", "remember", "step", "early",
    "hold", "west", "ground", "interest", "reach", "fast", "verb", "sing", "listen", "six",
    "table", "travel", "less", "morning", "ten", "simple", "several", "vowel", "toward", "war",
    "lay", "against", "pattern", "slow", "center", "love", "person", "money", "serve", "appear",
    "road", "map", "rain", "rule", "govern", "pull", "cold", "notice", "voice", "energy", "hunt",
    "probable", "bed", "brother", "egg", "ride", "cell", "believe", "perhaps", "pick", "sudden",
    "count", "reason", "square", "moment", "develop", "catch", "sleep", "wonder", "machine",
    "program", "system", "process", "method", "function", "variable", "state", "memory", "thread",
    "context", "module", "index", "query", "cache", "result", "error", "value", "type", "struct",
    "enum", "crate", "trait", "impl", "async", "await", "spawn", "channel", "future",
];

const TAGS: &[&str] = &[
    "draft",
    "review",
    "published",
    "archived",
    "idea",
    "reference",
    "meeting",
    "research",
    "personal",
    "work",
    "philosophy",
    "science",
    "technology",
    "design",
    "writing",
    "reading",
    "project",
    "task",
    "bug",
    "feature",
    "note",
    "log",
    "journal",
    "summary",
];

const STATUSES: &[&str] = &["active", "paused", "done", "dropped", "waiting"];

const FOLDER_NAMES: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
    "lambda", "mu", "nu", "xi", "omicron", "pi",
];

pub struct VaultConfig {
    pub projects: usize,
    pub daily_notes: usize,
    pub notes: usize,
    pub references: usize,
    pub seed: u64,
}

pub fn generate_vault(root: &Path, config: &VaultConfig) -> Vec<PathBuf> {
    let mut rng = StdRng::seed_from_u64(config.seed);
    let mut all_paths: Vec<PathBuf> = Vec::new();

    for (i, folder) in FOLDER_NAMES
        .iter()
        .cycle()
        .take(config.projects)
        .enumerate()
    {
        let name = if i < FOLDER_NAMES.len() {
            folder.to_string()
        } else {
            format!("{folder}-{i}")
        };
        all_paths.push(root.join("projects").join(format!("{name}.md")));
    }
    for i in 0..config.daily_notes {
        let month = i.rem_euclid(12).saturating_add(1);
        let day = i.rem_euclid(28).saturating_add(1);
        let cycle = i.div_euclid(336);
        let name = if cycle == 0 {
            format!("2026-{month:02}-{day:02}")
        } else {
            format!("2026-{month:02}-{day:02}-{cycle}")
        };
        all_paths.push(root.join("daily").join(format!("{name}.md")));
    }
    for i in 0..config.notes {
        all_paths.push(root.join("notes").join(format!("note-{i:04}.md")));
    }
    for i in 0..config.references {
        all_paths.push(root.join("references").join(format!("ref-{i:04}.md")));
    }

    let contents: Vec<(PathBuf, String)> = all_paths
        .iter()
        .map(|path| {
            let stem = file_stem(path);
            let content = generate_file(stem, &all_paths, &mut rng);
            (path.clone(), content)
        })
        .collect();

    write_fixture_files(&contents);

    all_paths
}

fn write_fixture_files(contents: &[(PathBuf, String)]) {
    for (path, content) in contents {
        if let Some(parent) = path.parent() {
            // The temp vault root is recreated fresh per bench run and is writable.
            #[allow(clippy::expect_used)]
            fs::create_dir_all(parent).expect("failed to create fixture directory");
        }
        // Writing into the freshly created temp vault root cannot fail.
        #[allow(clippy::expect_used)]
        fs::write(path, content).expect("failed to write fixture file");
    }
}

fn file_stem(path: &Path) -> &str {
    // Every fixture path ends in a UTF-8 ".md" file name.
    #[allow(clippy::expect_used)]
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("fixture paths always end in a UTF-8 file stem");
    stem
}

fn generate_file(name: &str, all_paths: &[PathBuf], rng: &mut StdRng) -> String {
    let link_count = rng.random_range(3..=15);
    let links = generate_links(all_paths, rng, link_count);
    let word_count = rng.random_range(200..=2000);
    let frontmatter = generate_frontmatter(name, rng);
    let body = generate_body(rng, word_count, &links);

    format!("{frontmatter}\n{body}")
}

fn generate_frontmatter(name: &str, rng: &mut StdRng) -> String {
    let year = rng.random_range(2024..=2025);
    let month = rng.random_range(1..=12);
    let day = rng.random_range(1..=28);

    let tag_count = rng.random_range(1..=4);
    let tags: Vec<&str> = (0..tag_count)
        .map(|_| TAGS.choose(rng).copied().unwrap_or("draft"))
        .collect();
    let tags_str = tags.join(", ");

    let status = STATUSES.choose(rng).copied().unwrap_or("active");
    let priority = rng.random_range(1..=5);

    let mut fm = format!(
        "---\ntitle: \"{name}\"\ncreated: {year}-{month:02}-{day:02}\ntags: [{tags_str}]\nstatus: {status}\npriority: {priority}"
    );

    if rng.random_bool(0.4)
        && let Err(error) = write!(fm, "\naliases: [\"{name}-v2\", \"{name}-draft\"]")
    {
        eprintln!("Failed to write fixture frontmatter: {error}");
    }
    if rng.random_bool(0.3)
        && let Err(error) = write!(fm, "\nsource: \"https://example.com/{name}\"")
    {
        eprintln!("Failed to write fixture frontmatter: {error}");
    }
    if rng.random_bool(0.3)
        && let Err(error) = write!(fm, "\nreviewed: {}", rng.random_bool(0.5))
    {
        eprintln!("Failed to write fixture frontmatter: {error}");
    }

    fm.push_str("\n---");
    fm
}

fn generate_links(all_paths: &[PathBuf], rng: &mut StdRng, count: usize) -> Vec<String> {
    let valid_count = count.saturating_mul(7).div_ceil(10);
    let dangling_count = count.saturating_div(5);
    let alias_count = count
        .saturating_sub(valid_count)
        .saturating_sub(dangling_count);

    let mut links = Vec::with_capacity(count);

    for _ in 0..valid_count {
        let target = pick_target(all_paths, rng);
        let stem = file_stem(target);
        links.push(format!("[[{stem}]]"));
    }

    for _ in 0..dangling_count {
        links.push(format!("[[Ghost-{:08x}]]", rng.random::<u32>()));
    }

    for _ in 0..alias_count {
        let target = pick_target(all_paths, rng);
        let stem = file_stem(target);
        if rng.random_bool(0.5) {
            links.push(format!("[[{stem}|see here]]"));
        } else {
            links.push(format!("[[{stem}#Section]]"));
        }
    }

    links.shuffle(rng);
    links
}

fn pick_target<'a>(all_paths: &'a [PathBuf], rng: &mut StdRng) -> &'a Path {
    // Bench fixtures always generate at least one file, so `choose` never returns `None`.
    #[allow(clippy::expect_used)]
    let target = all_paths
        .choose(rng)
        .map(PathBuf::as_path)
        .expect("fixture vault always contains at least one file");
    target
}

fn generate_body(rng: &mut StdRng, word_count: usize, links: &[String]) -> String {
    let mut body = String::with_capacity(word_count.saturating_mul(6));
    let mut link_idx = 0;
    let link_interval = if links.is_empty() {
        usize::MAX
    } else {
        word_count
            .checked_div(links.len().saturating_add(1))
            .unwrap_or(0)
    };

    let has_code_fence = rng.random_bool(0.3);
    let code_fence_start = if has_code_fence {
        Some(rng.random_range(50..word_count.saturating_sub(50)))
    } else {
        None
    };
    let code_fence_end = code_fence_start.map(|s| s.saturating_add(rng.random_range(20..60)));

    for i in 0..word_count {
        if i > 0 && i.rem_euclid(link_interval) == 0 && link_idx < links.len() {
            if let Some(link) = links.get(link_idx) {
                body.push_str(link);
            }
            body.push(' ');
            link_idx = link_idx.saturating_add(1);
            continue;
        }

        if code_fence_start == Some(i) {
            body.push_str("```\n");
        }

        body.push_str(WORDS.choose(rng).copied().unwrap_or("the"));
        body.push(' ');

        if code_fence_end == Some(i) {
            body.push_str("\n```\n");
        }

        if i % 80 == 79 {
            body.push('\n');
        }
    }

    if let Some(end) = code_fence_end
        && end >= word_count
    {
        body.push_str("\n```\n");
    }

    body
}
