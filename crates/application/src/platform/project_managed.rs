use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn validate_company_project_creation_for_human(
        &self,
        input: CreateCompanyProjectForHumanInput,
    ) -> AppResult<()> {
        self.prepare_company_project_for_human(input).map(|_| ())
    }

    pub fn create_managed_company_project_for_human(
        &self,
        input: CreateManagedCompanyProjectForHumanInput,
    ) -> AppResult<(CompanyProjectView, CompanyProjectGitAdminView)> {
        let human_user_id = input.project.human_user_id;
        let project_creation = self.prepare_company_project_for_human(input.project)?;
        let project_id = project_creation.project.id;
        let now = now_utc();
        let git_config = build_project_git_config(
            project_id,
            input.remote_url,
            input.host_local_path,
            Some(input.default_branch),
            Some(input.auth_profile),
            input.allow_agent_push,
            Some(input.branch_prefix),
            None,
            Some(human_user_id),
            None,
            Some(human_user_id),
            None,
            now,
        )?;
        let (project, git) =
            self.complete_company_project_creation(project_creation, Some(git_config))?;
        let git = git.ok_or_else(|| {
            AppError::Internal("managed project commit did not return its Git configuration".into())
        })?;
        Ok((project, company_project_git_admin_view(git)))
    }
}
