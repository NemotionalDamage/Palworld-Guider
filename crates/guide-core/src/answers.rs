use game_knowledge::{Confidence, ConflictRecord, Provenance, ReviewStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnswerStatus {
    Ok,
    Unknown,
    Ambiguous,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProvenanceSummary {
    pub source_id: String,
    pub applicable_game_version: String,
    pub retrieved_on: String,
    pub reviewer: String,
    pub review_status: String,
    pub confidence: String,
    pub change_risk: Option<String>,
}

impl ProvenanceSummary {
    pub fn new(provenance: &Provenance) -> Self {
        Self {
            source_id: provenance.source_id.clone(),
            applicable_game_version: provenance.applicable_game_version.clone(),
            retrieved_on: provenance.retrieved_on.clone(),
            reviewer: provenance.reviewer.clone(),
            review_status: review_status_value(&provenance.review_status).to_string(),
            confidence: confidence_value(&provenance.confidence).to_string(),
            change_risk: provenance.change_risk.clone(),
        }
    }
}

fn review_status_value(status: &ReviewStatus) -> &'static str {
    match status {
        ReviewStatus::Candidate => "candidate",
        ReviewStatus::Reviewed => "reviewed",
        ReviewStatus::Retired => "retired",
    }
}

fn confidence_value(confidence: &Confidence) -> &'static str {
    match confidence {
        Confidence::VerifiedTarget => "verified_target",
        Confidence::Official => "official",
        Confidence::ReviewedSecondary => "reviewed_secondary",
        Confidence::Community => "community",
        Confidence::Conflicted => "conflicted",
        Confidence::Unknown => "unknown",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionInfo {
    pub knowledge_version: String,
    pub configured_game_version: Option<String>,
    pub matches: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuideAnswer<T> {
    pub status: AnswerStatus,
    pub data: Option<T>,
    pub provenance: Vec<ProvenanceSummary>,
    pub version: VersionInfo,
    pub uncertainty: Vec<String>,
    pub errors: Vec<String>,
}

pub(crate) struct AnswerContext<'a> {
    pub configured_game_version: Option<&'a str>,
}

impl AnswerContext<'_> {
    pub(crate) fn answer<T>(
        &self,
        status: AnswerStatus,
        data: Option<T>,
        provenances: Vec<&Provenance>,
        conflicts: Vec<&ConflictRecord>,
    ) -> GuideAnswer<T> {
        let mut summaries = BTreeSet::new();
        for provenance in provenances {
            summaries.insert(ProvenanceSummary::new(provenance));
        }
        let summaries: Vec<_> = summaries.into_iter().collect();

        let versions: BTreeSet<_> = summaries
            .iter()
            .map(|summary| summary.applicable_game_version.clone())
            .collect();
        let knowledge_version = match versions.len() {
            0 => "unknown".to_string(),
            1 => versions
                .into_iter()
                .next()
                .expect("one version is present")
                .to_string(),
            _ => "mixed".to_string(),
        };
        let matches = match self.configured_game_version {
            Some(configured) => {
                knowledge_version != "mixed"
                    && knowledge_version != "unknown"
                    && knowledge_version == configured
            }
            None => true,
        };

        let mut uncertainty = Vec::new();
        if !matches {
            uncertainty.push(format!(
                "knowledge version {knowledge_version} does not match configured game version {}",
                self.configured_game_version.unwrap_or("unknown")
            ));
        }
        for conflict in conflicts {
            uncertainty.push(format!(
                "conflicting values remain for {}: {}",
                conflict.subject_id,
                conflict.values.join(" vs ")
            ));
        }

        GuideAnswer {
            status,
            data,
            provenance: summaries,
            version: VersionInfo {
                knowledge_version,
                configured_game_version: self.configured_game_version.map(str::to_string),
                matches,
            },
            uncertainty,
            errors: Vec::new(),
        }
    }

    pub(crate) fn unknown<T>(&self, message: impl Into<String>) -> GuideAnswer<T> {
        let mut answer = self.answer(AnswerStatus::Unknown, None, Vec::new(), Vec::new());
        answer.uncertainty.push(message.into());
        answer
    }

    pub(crate) fn ambiguous<T>(&self, message: impl Into<String>) -> GuideAnswer<T> {
        let mut answer = self.answer(AnswerStatus::Ambiguous, None, Vec::new(), Vec::new());
        answer.uncertainty.push(message.into());
        answer
    }

    pub(crate) fn error<T>(&self, message: impl Into<String>) -> GuideAnswer<T> {
        let mut answer = self.answer(AnswerStatus::Error, None, Vec::new(), Vec::new());
        answer.errors.push(message.into());
        answer
    }
}

impl<T> GuideAnswer<T> {
    pub(crate) fn map_data<U>(
        self,
        transform: impl FnOnce(Option<T>) -> Option<U>,
    ) -> GuideAnswer<U> {
        GuideAnswer {
            status: self.status,
            data: transform(self.data),
            provenance: self.provenance,
            version: self.version,
            uncertainty: self.uncertainty,
            errors: self.errors,
        }
    }
}
