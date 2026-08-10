use super::*;

pub trait ProjectPlatformRepository: Send + Sync {
    fn complete_company_project_creation(
        &self,
        _bundle: CompanyProjectCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company projects are not supported by this repository".into(),
        ))
    }
    fn complete_managed_company_project_creation(
        &self,
        _bundle: ManagedCompanyProjectCreationBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "managed company projects are not supported by this repository".into(),
        ))
    }
    fn save_project_provisioning_cleanup_job(
        &self,
        _job: ProjectProvisioningCleanupJob,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project provisioning cleanup is not supported by this repository".into(),
        ))
    }
    fn claim_due_project_provisioning_cleanup_job(
        &self,
        _now: DateTime<Utc>,
        _lease_expires_at: DateTime<Utc>,
    ) -> AppResult<Option<ProjectProvisioningCleanupJob>> {
        Ok(None)
    }
    fn retry_project_provisioning_cleanup_job(
        &self,
        _job_id: Uuid,
        _error: String,
        _next_attempt_at: DateTime<Utc>,
        _updated_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project provisioning cleanup is not supported by this repository".into(),
        ))
    }
    fn complete_project_provisioning_cleanup_job(
        &self,
        _job_id: Uuid,
        _completed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "project provisioning cleanup is not supported by this repository".into(),
        ))
    }
    fn get_company_project(&self, _project_id: Uuid) -> Option<CompanyProject> {
        None
    }
    fn get_company_project_result(&self, project_id: Uuid) -> AppResult<Option<CompanyProject>> {
        Ok(self.get_company_project(project_id))
    }
    fn list_company_projects(&self, _company_id: Uuid) -> Vec<CompanyProject> {
        Vec::new()
    }
    fn list_company_projects_result(&self, company_id: Uuid) -> AppResult<Vec<CompanyProject>> {
        Ok(self.list_company_projects(company_id))
    }
    fn list_company_project_page(
        &self,
        company_id: Uuid,
        after_project_id: Option<Uuid>,
        limit: usize,
    ) -> AppResult<CursorPage<CompanyProject>> {
        cursor_page_by_id(
            self.list_company_projects_result(company_id)?,
            after_project_id,
            limit,
            |project| project.id,
        )
    }
    fn save_company_project_git_config(&self, _config: CompanyProjectGitConfig) -> AppResult<()> {
        Err(AppError::Validation(
            "company project Git configuration is not supported by this repository".into(),
        ))
    }
    fn get_company_project_git_config(&self, _project_id: Uuid) -> Option<CompanyProjectGitConfig> {
        None
    }
    fn delete_company_project_git_config(&self, _project_id: Uuid) -> AppResult<()> {
        Err(AppError::Validation(
            "company project Git configuration is not supported by this repository".into(),
        ))
    }
    fn save_company_project_rule(&self, _rule: CompanyProjectRule) -> AppResult<()> {
        Err(AppError::Validation(
            "company project rules are not supported by this repository".into(),
        ))
    }
    fn get_company_project_rule(&self, _project_id: Uuid) -> Option<CompanyProjectRule> {
        None
    }
    fn replace_company_project_assets(
        &self,
        _project_id: Uuid,
        _assets: Vec<CompanyProjectAsset>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project assets are not supported by this repository".into(),
        ))
    }
    fn list_company_project_assets(&self, _project_id: Uuid) -> Vec<CompanyProjectAsset> {
        Vec::new()
    }
    fn save_company_project_asset_refresh_config(
        &self,
        _config: CompanyProjectAssetRefreshConfig,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project asset refresh is not supported by this repository".into(),
        ))
    }
    fn get_company_project_asset_refresh_config(
        &self,
        _project_id: Uuid,
    ) -> Option<CompanyProjectAssetRefreshConfig> {
        None
    }
    fn claim_due_company_project_asset_refresh(
        &self,
        _agent_id: Uuid,
        _now: DateTime<Utc>,
    ) -> AppResult<Option<CompanyProjectAssetRefreshConfig>> {
        Ok(None)
    }
    fn mark_company_project_asset_refresh_completed(
        &self,
        _project_id: Uuid,
        _completed_at: DateTime<Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project asset refresh is not supported by this repository".into(),
        ))
    }
    fn update_company_project(&self, _project: CompanyProject) -> AppResult<()> {
        Err(AppError::Validation(
            "company project updates are not supported by this repository".into(),
        ))
    }
    fn update_company_project_metadata(
        &self,
        _project: CompanyProject,
        _project_group_title: String,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project metadata updates are not supported by this repository".into(),
        ))
    }
    fn get_company_project_member(
        &self,
        _project_id: Uuid,
        _agent_id: Uuid,
    ) -> Option<CompanyProjectMember> {
        None
    }
    fn list_company_project_members(&self, _project_id: Uuid) -> Vec<CompanyProjectMember> {
        Vec::new()
    }
    fn complete_company_project_member_add(
        &self,
        _bundle: CompanyProjectMemberAddBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project membership is not supported by this repository".into(),
        ))
    }
    fn complete_company_project_owner_transfer(
        &self,
        _bundle: CompanyProjectOwnerTransferBundle,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project owner transfer is not supported by this repository".into(),
        ))
    }
    fn complete_company_project_member_remove(
        &self,
        _project_id: Uuid,
        _agent_id: Uuid,
        _conversation_id: Uuid,
        _left_at: chrono::DateTime<chrono::Utc>,
    ) -> AppResult<()> {
        Err(AppError::Validation(
            "company project membership is not supported by this repository".into(),
        ))
    }
}
