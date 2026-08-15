use super::*;

#[derive(Clone)]
pub(super) struct PlatformRunCancellationHandler {
    pub(super) platform: TriggerPlatform,
    pub(super) realtime_sender: tokio::sync::broadcast::Sender<CompanyRealtimeSignal>,
    pub(super) company_id: Uuid,
    pub(super) trigger_config_id: Uuid,
    pub(super) agent_id: Uuid,
    pub(super) project_id: Option<Uuid>,
    pub(super) reason: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl CodexCancellationHandler for PlatformRunCancellationHandler {
    fn should_cancel(&self) -> bool {
        match self.platform.is_agent_codex_trigger_active(self.agent_id) {
            Ok(false) => {
                self.remember_reason(
                    "Codex run cancelled because the Agent Trigger was paused by Human",
                );
                return true;
            }
            Ok(true) => {}
            Err(error) => {
                tracing::error!(
                    agent_id = %self.agent_id,
                    error = %error,
                    "failed to read Agent Trigger state; cancelling the Codex run defensively"
                );
                self.remember_reason(
                    "Codex run cancelled because the Agent Trigger state could not be verified",
                );
                return true;
            }
        }
        let Some(project_id) = self.project_id else {
            return false;
        };
        match self.platform.is_company_project_paused(project_id) {
            Ok(true) => {
                self.remember_reason("Codex run cancelled because the project was paused");
                true
            }
            Ok(false) => false,
            Err(error) => {
                tracing::error!(
                    project_id = %project_id,
                    error = %error,
                    "failed to read project pause state; cancelling the Codex run defensively"
                );
                self.remember_reason(
                    "Codex run cancelled because the project state could not be verified",
                );
                true
            }
        }
    }

    async fn wait_for_cancellation(&self) {
        let mut receiver = self.realtime_sender.subscribe();
        if self.should_cancel() {
            return;
        }
        let fallback = tokio::time::sleep(StdDuration::from_secs(30));
        tokio::pin!(fallback);
        loop {
            tokio::select! {
                signal = receiver.recv() => match signal {
                    Ok(signal) if self.signal_can_change_cancellation(&signal) => {
                        if self.should_cancel() {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        if self.should_cancel() {
                            return;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        if self.should_cancel() {
                            return;
                        }
                        fallback.as_mut().reset(
                            tokio::time::Instant::now() + StdDuration::from_secs(5),
                        );
                    }
                },
                _ = &mut fallback => {
                    if self.should_cancel() {
                        return;
                    }
                    fallback.as_mut().reset(
                        tokio::time::Instant::now() + StdDuration::from_secs(30),
                    );
                }
            }
        }
    }

    fn cancellation_reason(&self) -> String {
        self.reason
            .lock()
            .expect("cancellation reason lock poisoned")
            .clone()
            .unwrap_or_else(|| "Codex run cancelled because the project was paused".into())
    }
}

impl PlatformRunCancellationHandler {
    fn remember_reason(&self, reason: &str) {
        *self
            .reason
            .lock()
            .expect("cancellation reason lock poisoned") = Some(reason.into());
    }

    fn signal_can_change_cancellation(&self, signal: &CompanyRealtimeSignal) -> bool {
        signal_can_change_cancellation(
            self.company_id,
            self.trigger_config_id,
            self.project_id,
            signal,
        )
    }
}

fn signal_can_change_cancellation(
    company_id: Uuid,
    trigger_config_id: Uuid,
    project_id: Option<Uuid>,
    signal: &CompanyRealtimeSignal,
) -> bool {
    if signal.company_id != company_id {
        return false;
    }
    match (
        signal.event_type.as_deref(),
        signal.aggregate_type.as_deref(),
        signal.aggregate_id,
    ) {
        (Some("codex.trigger.updated"), Some("agent_codex_trigger_config"), Some(config_id)) => {
            config_id == trigger_config_id
        }
        (Some("project.updated"), Some("project"), Some(changed_project_id)) => {
            project_id == Some(changed_project_id)
        }
        (None, _, _) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(
        company_id: Uuid,
        event_type: Option<&str>,
        aggregate_type: Option<&str>,
        aggregate_id: Option<Uuid>,
    ) -> CompanyRealtimeSignal {
        CompanyRealtimeSignal {
            sequence_id: 1,
            company_id,
            event_type: event_type.map(str::to_string),
            aggregate_type: aggregate_type.map(str::to_string),
            aggregate_id,
            execution_requested: None,
        }
    }

    #[test]
    fn cancellation_signals_are_scoped_to_the_active_trigger_and_project() {
        let company_id = Uuid::new_v4();
        let config_id = Uuid::new_v4();
        let project_id = Uuid::new_v4();
        assert!(signal_can_change_cancellation(
            company_id,
            config_id,
            Some(project_id),
            &signal(
                company_id,
                Some("codex.trigger.updated"),
                Some("agent_codex_trigger_config"),
                Some(config_id),
            ),
        ));
        assert!(signal_can_change_cancellation(
            company_id,
            config_id,
            Some(project_id),
            &signal(
                company_id,
                Some("project.updated"),
                Some("project"),
                Some(project_id),
            ),
        ));
        assert!(!signal_can_change_cancellation(
            company_id,
            config_id,
            Some(project_id),
            &signal(
                company_id,
                Some("codex.trigger.updated"),
                Some("agent_codex_trigger_config"),
                Some(Uuid::new_v4()),
            ),
        ));
        assert!(!signal_can_change_cancellation(
            company_id,
            config_id,
            Some(project_id),
            &signal(
                company_id,
                Some("project.updated"),
                Some("project"),
                Some(Uuid::new_v4()),
            ),
        ));
    }

    #[test]
    fn legacy_company_signal_requests_a_defensive_recheck() {
        let company_id = Uuid::new_v4();
        assert!(signal_can_change_cancellation(
            company_id,
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            &signal(company_id, None, None, None),
        ));
    }
}
