//! Lexicon data. This module is intentionally boring: lists of phrases that
//! rule data references. Add a new word or asset noun here; never add matching
//! control flow. All phrases are lowercase and matched ASCII-case-insensitively.

/// Verbs that transfer a right away from the actor.
pub const WAIVER_VERBS: &[&str] = &[
    "waive",
    "waives",
    "waived",
    "waiving",
    "waiver of",
    // Plain-language synonyms are only waivers when they take a right as their
    // object; "give up your membership" is not.
    "give up the right",
    "give up your right",
    "give up any right",
    "gives up the right",
    "giving up the right",
    "giving up your right",
    "giving up any right",
    "forgo the right",
    "forgo your right",
    "forgo any right",
    "forego any right",
    "relinquish the right",
    "relinquish your right",
    "relinquish any right",
];

/// Class/collective/representative action language.
pub const CLASS_ACTIONS: &[&str] = &[
    "class action",
    "class-action",
    "class proceeding",
    "class or collective action",
    "collective action",
    "representative action",
    "class member",
    "class arbitration",
    "representative proceeding",
    "purported class",
];

/// Jury language that can be waived.
pub const JURY_TERMS: &[&str] = &[
    "jury trial",
    "trial by jury",
    "jury service",
    "right to a jury",
];

/// Judicial-forum language.
pub const COURT_TERMS: &[&str] = &[
    "right to sue",
    "right to file a claim",
    "right to bring a claim",
    "court",
    "judicial",
    "lawsuit",
    "legal proceeding",
    "judge",
    "litigate",
];

/// Language that actually mandates arbitration, as opposed to describing it.
pub const MANDATORY_ARBITRATION: &[&str] = &[
    "binding arbitration",
    "mandatory arbitration",
    "shall be resolved by arbitration",
    "shall be resolved exclusively by arbitration",
    "shall be resolved by binding arbitration",
    "must be resolved by arbitration",
    "must be resolved through arbitration",
    "shall be arbitrated",
    "must be arbitrated",
    "agree to arbitrate",
    "agrees to arbitrate",
    "you agree that any dispute",
    "you agree that all disputes",
    "requires you to arbitrate",
    "required to arbitrate",
    "settled by arbitration",
    "referred to arbitration",
    "submitted to arbitration",
    "resolved by arbitration",
    "resolved through arbitration",
    "resolved exclusively through arbitration",
    "arbitration shall be the exclusive",
    "decided by an arbitrator",
];

/// Modal or consensual arbitration language. A clause that only permits
/// arbitration, or makes it depend on mutual consent, is not mandatory.
pub const VOLUNTARY_ARBITRATION: &[&str] = &[
    "may be settled by arbitration",
    "may be referred to arbitration",
    "may be submitted to arbitration",
    "may be resolved by arbitration",
    "if both parties agree",
    "if the parties agree",
    "by mutual agreement",
    "if you prefer",
    "if you wish",
    "you may choose to",
    "arbitration is optional",
    "arbitration is voluntary",
];

/// References to an arbitration term from severability or interpretation
/// prose. The sentence is about the clause, not the clause itself.
pub const SEVERABILITY_REFERENCES: &[&str] = &[
    "found unenforceable",
    "found to be unenforceable",
    "held unenforceable",
    "deemed unenforceable",
    "is found invalid",
    "is held invalid",
];

/// Something the arbitration clause can be about.
pub const DISPUTE_NEXUS: &[&str] = &["dispute", "claim", "controversy", "lawsuit"];

/// A restriction that bars collective proceedings.
pub const CLASS_ACTION_BARS: &[&str] = &[
    "not permitted",
    "prohibited",
    "may not",
    "cannot",
    "shall not",
    "must be brought individually",
    "brought only on an individual basis",
    "individual basis only",
    "only on an individual basis",
    "individual capacity",
    "only individually",
    "not as a plaintiff",
    "agree not to",
    "will not be a class",
    "neither party may",
    "no dispute may",
    "no claim may",
];

/// A party or proceeding that a class bar restricts. Without one, "cannot" next
/// to "class action" is ordinary prose ("support cannot answer questions about
/// class action settlements").
pub const CLASS_BAR_SUBJECTS: &[&str] = &[
    "you",
    "claim",
    "dispute",
    "party",
    "parties",
    "arbitrator",
    "under this agreement",
    "under these terms",
];

/// A waiver by the provider is not a user-facing predatory clause.
pub const PROVIDER_WAIVERS: &[&str] = &[
    "we waive",
    "we hereby waive",
    "we irrevocably waive",
    "company waives",
    "provider waives",
    "we agree to waive",
    "we will never waive",
    "we give up",
    "company gives up",
];

/// Compliant boilerplate that must suppress an apparent hostile footprint.
pub const COMPLIANCE_GUARDS: &[&str] = &[
    "does not waive",
    "do not waive",
    "not waive",
    "never waive",
    "no waiver",
    "nothing in this",
    "nothing in these",
    "nothing herein",
    "no provision of",
    "not be deemed a waiver",
    "does not apply",
    "does not prevent",
    "does not limit",
    "will not be construed",
    "shall not be construed",
    "is not intended to",
    "not give up",
    "not giving up",
];

/// Extra evidence that does not gate a finding.
pub const ARBITRATION_AMPLIFIERS: &[&str] =
    &["plaintiff", "class member", "representative", "collective"];

/// Unilateral discretion markers. These raise `mu` in scoring.
pub const UNILATERAL_DISCRETION: &[&str] = &[
    "sole discretion",
    "at any time",
    "without notice",
    "at our option",
    "in our discretion",
    "unilaterally",
];

/// Negations that make a base clause compliant before a carve-out marker.
pub const COMPLIANCE_NEGATIONS: &[&str] = &[
    "not",
    "never",
    "no",
    "nothing",
    "does not",
    "do not",
    "will not",
    "shall not",
    "is not",
    "are not",
];

/// Actions that constitute exercising an opt-out.
pub const OPT_OUT_ACTIONS: &[&str] = &[
    "opt out",
    "opt-out",
    "optout",
    "reject this arbitration",
    "reject the arbitration",
    "reject arbitration",
    "exclude yourself",
    "written notice",
    "written objection",
    "notify us in writing",
    "send written notice",
    "provide written notice",
    "email us",
    "email written notice",
    "mail written notice",
];

/// Explicit windows. Numeric windows are additionally detected by shape.
pub const OPT_OUT_WINDOW_PHRASES: &[&str] = &[
    "at any time",
    "within a reasonable time",
    "before the effective date",
    "within thirty (30) days",
    "within sixty (60) days",
    "within ninety (90) days",
    "no later than thirty (30) days",
    "no later than sixty (60) days",
];

/// Word-number day windows. Digits are detected separately.
pub const OPT_OUT_DAY_WORDS: &[&str] = &[
    "ten days",
    "fifteen days",
    "twenty days",
    "thirty days",
    "forty-five days",
    "sixty days",
    "ninety days",
];

/// Subjects that make an opt-out clause about something other than dispute
/// resolution. A neighbouring opt-out naming one of these is never associated
/// with a finding in another clause: "opt out of marketing emails" is not the
/// arbitration opt-out, however close it sits.
pub const OPT_OUT_FOREIGN_SUBJECTS: &[&str] = &[
    "marketing",
    "promotional",
    "newsletter",
    "advertising",
    "advertisement",
    "cookie",
    "tracking",
    "analytics",
    "profiling",
    "personal information",
    "personal data",
    "data sharing",
    "sharing of your",
    "sale of your",
    "text messages",
    "auto-renewal",
    "automatic renewal",
];

/// Subjects that tie a neighbouring opt-out to dispute resolution. When any
/// candidate in the association window names one, only those candidates count.
pub const OPT_OUT_DISPUTE_SUBJECTS: &[&str] = &[
    "arbitration",
    "arbitrate",
    "dispute resolution",
    "class action waiver",
    "this provision",
    "this section",
];
