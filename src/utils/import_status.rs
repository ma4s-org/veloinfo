use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

static SUCCESSES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn successes() -> &'static Mutex<HashSet<String>> {
    SUCCESSES.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Remet à zéro le suivi avant une passe d'import.
pub fn reset() {
    if let Ok(mut successes) = successes().lock() {
        successes.clear();
    }
}

/// Marque une source comme importée avec succès.
///
/// Appelé par les fonctions `replace_*` après le commit SQL, et par les
/// modules lorsqu'une source n'a simplement aucun travail en cours.
pub fn mark_success(source: &str) {
    if let Ok(mut successes) = successes().lock() {
        successes.insert(source.to_string());
    }
}

/// Sources attendues qui n'ont pas été marquées comme réussies.
pub fn failures<'a>(expected: &'a [&'a str]) -> Vec<&'a str> {
    let Ok(successes) = successes().lock() else {
        return expected.to_vec();
    };

    expected
        .iter()
        .copied()
        .filter(|source| !successes.contains(*source))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signale_les_sources_qui_n_ont_pas_reussi() {
        reset();
        mark_success("montreal");
        mark_success("laval");

        assert_eq!(
            failures(&["montreal", "laval", "gatineau"]),
            vec!["gatineau"]
        );
    }
}
