//! Coordinator safety predicates, independent of native capture and storage.
#[derive(Default)]
pub(crate) struct FirstFailure(Option<String>);
#[derive(Default)]
pub(crate) struct FailureLedger {
    first: FirstFailure,
    global: Option<String>,
    sources: std::collections::HashMap<String, String>,
}
impl FailureLedger {
    pub(crate) fn record(&mut self, source: Option<&str>, reason: String) -> String {
        self.first.record(reason.clone());
        match source {
            Some(source) => {
                self.sources.entry(source.into()).or_insert(reason);
            }
            None => {
                self.global.get_or_insert(reason);
            }
        }
        self.first.get().expect("failure just recorded").into()
    }
    pub(crate) fn get(&self) -> Option<&str> {
        self.first.get()
    }
    pub(crate) fn for_source(&self, source: &str) -> Option<&str> {
        self.sources
            .get(source)
            .map(String::as_str)
            .or(self.global.as_deref())
    }
    pub(crate) fn global(&self) -> Option<&str> {
        self.global.as_deref()
    }
}
impl FirstFailure {
    pub(crate) fn record(&mut self, reason: String) {
        if self.0.is_none() {
            self.0 = Some(reason);
        }
    }
    pub(crate) fn get(&self) -> Option<&str> {
        self.0.as_deref()
    }
}
pub(crate) fn can_complete(
    ready: bool,
    stopped: bool,
    drained: bool,
    known: bool,
    nonempty: bool,
    fault: bool,
    strict: bool,
) -> bool {
    ready && stopped && drained && known && nonempty && !fault && strict
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_failure_survives_shutdown_errors() {
        let mut first = FirstFailure::default();
        first.record("route changed".into());
        first.record("writer teardown failed".into());
        assert_eq!(first.get(), Some("route changed"));
    }
    #[test]
    fn complete_requires_all_sources_and_final_native_admission() {
        assert!(can_complete(true, true, true, true, true, false, true));
        for missing in 0..6 {
            let mut evidence = [true; 6];
            evidence[missing] = false;
            assert!(!can_complete(
                evidence[0],
                evidence[1],
                evidence[2],
                evidence[3],
                evidence[4],
                false,
                evidence[5]
            ));
        }
        assert!(!can_complete(true, true, true, true, true, true, true));
    }
}
