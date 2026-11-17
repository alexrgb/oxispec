//! The kinds of documents a project can keep, and the `documents` list in the config that
//! fixes which of them it uses (`oxispec onboard`).

/// A kind of document: the node type, the module it needs and the command that writes it.
pub struct Kind {
    /// The node type, such as `adr`.
    pub ty: &'static str,
    /// What it is called to a person.
    pub label: &'static str,
    /// The module that has to be on.
    pub module: &'static str,
    /// The `oxispec new` command, without the title.
    pub command: &'static str,
}

pub const KINDS: &[Kind] = &[
    Kind {
        ty: "requirement",
        label: "Requirement with scenarios",
        module: "specs",
        command: "new requirement DOMAIN",
    },
    Kind {
        ty: "adr",
        label: "Architecture decision record",
        module: "adrs",
        command: "new adr",
    },
    Kind {
        ty: "change",
        label: "Change set",
        module: "specs",
        command: "new change",
    },
    Kind {
        ty: "prd",
        label: "Product requirements document",
        module: "specs",
        command: "new prd",
    },
    Kind {
        ty: "test-plan",
        label: "Test plan",
        module: "tests",
        command: "new test-plan",
    },
    Kind {
        ty: "risk",
        label: "Risk",
        module: "risks",
        command: "new risk",
    },
    Kind {
        ty: "debt",
        label: "Tech debt item",
        module: "debt",
        command: "new debt",
    },
    Kind {
        ty: "vocabulary",
        label: "Vocabulary",
        module: "consistency",
        command: "new vocabulary",
    },
    Kind {
        ty: "entity",
        label: "Entity",
        module: "consistency",
        command: "new entity",
    },
    Kind {
        ty: "state-machine",
        label: "State machine",
        module: "statemachines",
        command: "new machine",
    },
    Kind {
        ty: "person",
        label: "C4 person",
        module: "architecture",
        command: "new architecture person",
    },
    Kind {
        ty: "system",
        label: "C4 software system",
        module: "architecture",
        command: "new architecture system",
    },
    Kind {
        ty: "container",
        label: "C4 container",
        module: "architecture",
        command: "new architecture container",
    },
    Kind {
        ty: "component",
        label: "C4 component",
        module: "architecture",
        command: "new architecture component",
    },
    Kind {
        ty: "deployment-environment",
        label: "C4 deployment environment",
        module: "architecture",
        command: "new architecture environment",
    },
];

pub fn kind(ty: &str) -> Option<&'static Kind> {
    KINDS.iter().find(|k| k.ty == ty)
}

/// What a project that asks for nothing else keeps.
pub const DEFAULT: &[&str] = &["requirement", "adr", "change"];
