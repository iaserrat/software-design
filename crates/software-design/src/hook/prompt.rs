//! Routes code design and review prompts to the matching skill.

/// A kind of request one of the plugin's skills serves, recognised by a verb
/// phrase together with a noun for code. Requiring both keeps "design a landing
/// page" or "review my essay" from matching.
struct Intent {
    skill: &'static str,
    verbs: &'static [&'static str],
    nouns: &'static [&'static str],
    why: &'static str,
}

/// Checked in order; the first match wins, so "review the design of X" is a review.
const INTENTS: [Intent; 2] = [
    Intent {
        skill: "software-design:review",
        verbs: &[
            "review",
            "design problem",
            "well designed",
            "well-designed",
            "red flag",
            "code smell",
            "without reading",
            "maintainab",
            "look over",
            "overcomplicated",
            "over-engineered",
            "tangled",
            "messy",
            "copy-paste",
            "duplicated code",
            "shallow",
            "just a wrapper",
        ],
        nouns: &[
            "code",
            "diff",
            "branch",
            "pr",
            "pull",
            "file",
            "module",
            "class",
            "api",
            "interface",
            "src",
            "crate",
            "package",
            "service",
            "controller",
            "function",
            "method",
            "change",
        ],
        why: "it finds red flags with file and line evidence, verifies each against the rule's exemptions, \
and tests public interfaces on a reader who sees only signatures and comments",
    },
    Intent {
        skill: "software-design:design",
        verbs: &[
            "design",
            "sketch",
            "structure",
            "shape",
            "architect",
            "what methods",
            "which methods",
            "lay out",
            "look like",
            "planning",
            "split",
        ],
        nouns: &[
            "class",
            "module",
            "api",
            "interface",
            "service",
            "library",
            "abstraction",
            "client",
            "composable",
            "struct",
            "trait",
            "crate",
            "package",
        ],
        why: "it compares independent designs and tests each interface on a reader who has not seen it",
    },
];

/// The context to add for a prompt one of the plugin's skills serves. Slash
/// commands are explicit choices and get none.
pub(super) fn nudge(prompt: &str) -> Option<String> {
    let prompt = prompt.to_lowercase();
    if prompt.trim_start().starts_with('/') {
        return None;
    }
    let words: Vec<&str> = prompt
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has_word = |noun: &str| {
        words
            .iter()
            .any(|w| *w == noun || w.strip_suffix('s') == Some(noun))
    };
    INTENTS
        .iter()
        .find(|intent| {
            intent.nouns.iter().any(|n| has_word(n))
                && intent.verbs.iter().any(|v| prompt.contains(v))
        })
        .map(|intent| {
            format!(
                "software-design: the {skill} skill serves this request. Before answering, and before \
exploring the repository, load the {skill} skill (in Claude Code, call the Skill tool with skill \"{skill}\"): {why}. The skill reads the code it \
needs itself.",
                skill = intent.skill,
                why = intent.why
            )
        })
}
