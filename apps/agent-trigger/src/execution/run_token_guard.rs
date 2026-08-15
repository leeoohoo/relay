use super::*;

pub(super) struct ManagedBrowserRunTokenGuard<'a> {
    runner: &'a CodexTriggerRunner,
    run_token: &'a str,
}

impl<'a> ManagedBrowserRunTokenGuard<'a> {
    pub(super) fn new(runner: &'a CodexTriggerRunner, run_token: &'a str) -> Self {
        Self { runner, run_token }
    }
}

impl Drop for ManagedBrowserRunTokenGuard<'_> {
    fn drop(&mut self) {
        self.runner.revoke_managed_browser_run_token(self.run_token);
    }
}
