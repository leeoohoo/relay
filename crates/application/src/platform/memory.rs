use super::*;

impl<R: PlatformRepository, V: OwnershipProofVerifier> PlatformApp<R, V> {
    pub fn remember_agent_memory(&self, input: RememberAgentMemoryInput) -> AppResult<AgentMemory> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        if membership.company_id != input.company_id {
            return Err(AppError::Unauthorized(
                "agent does not belong to the requested company".into(),
            ));
        }
        if let Some(project_id) = input.project_id {
            self.ensure_company_project_access(input.company_id, project_id, input.actor_agent_id)?;
        }
        let scope = input
            .scope
            .as_deref()
            .map(normalize_agent_memory_scope)
            .transpose()?
            .unwrap_or_else(|| {
                if input.session_id.is_some() {
                    AGENT_MEMORY_SCOPE_SESSION.into()
                } else if input.project_id.is_some() {
                    AGENT_MEMORY_SCOPE_PROJECT.into()
                } else {
                    AGENT_MEMORY_SCOPE_AGENT.into()
                }
            });
        let session_visibility = match scope.as_str() {
            AGENT_MEMORY_SCOPE_AGENT | AGENT_MEMORY_SCOPE_CONTROL => {
                if input.project_id.is_some() || input.session_id.is_some() {
                    return Err(AppError::Validation(
                        "agent and control memories cannot bind a project or session".into(),
                    ));
                }
                None
            }
            AGENT_MEMORY_SCOPE_PROJECT => {
                if input.project_id.is_none() || input.session_id.is_some() {
                    return Err(AppError::Validation(
                        "project memories require project_id and cannot bind session_id".into(),
                    ));
                }
                None
            }
            AGENT_MEMORY_SCOPE_SESSION => {
                let session_id = input.session_id.ok_or_else(|| {
                    AppError::Validation("session memories require session_id".into())
                })?;
                let session = self
                    .repo
                    .list_agent_codex_sessions(input.actor_agent_id, 100)
                    .into_iter()
                    .find(|session| session.id == session_id);
                if input.project_id.is_some() || session.is_none() {
                    return Err(AppError::Unauthorized(
                        "Agent cannot bind memory to the requested session".into(),
                    ));
                }
                Some(
                    if session.is_some_and(|session| {
                        session.session_kind == AGENT_CODEX_SESSION_KIND_CONTROL
                    }) {
                        AGENT_MEMORY_VISIBILITY_CONTROL
                    } else {
                        AGENT_MEMORY_VISIBILITY_WORKER
                    },
                )
            }
            _ => unreachable!("memory scope is normalized"),
        };
        let requested_memory_tier = normalize_agent_memory_tier(&input.memory_tier)?;
        let topic_key = normalize_agent_memory_topic_key(input.topic_key)?;
        let memory_type = normalize_agent_memory_type(&input.memory_type)?;
        let title = normalize_agent_memory_text(input.title, 200, "memory title", 1)?;
        let summary = normalize_agent_memory_summary(input.summary)?;
        let when_to_use = normalize_agent_memory_optional_text(
            input.when_to_use.unwrap_or_default(),
            1_000,
            "memory usage context",
        )?;
        let classification = classify_agent_memory(
            &requested_memory_tier,
            &memory_type,
            &title,
            &summary,
            &when_to_use,
        );
        let tags = normalize_agent_memory_tags(input.tags)?;
        let importance = input.importance.unwrap_or(3);
        if !(1..=5).contains(&importance) {
            return Err(AppError::Validation(
                "memory importance must be between 1 and 5".into(),
            ));
        }
        let confidence = input.confidence.unwrap_or(80);
        if !(0..=100).contains(&confidence) {
            return Err(AppError::Validation(
                "memory confidence must be between 0 and 100".into(),
            ));
        }
        validate_agent_memory_source_refs(&input.source_refs)?;
        if input
            .expires_at
            .is_some_and(|expires_at| expires_at <= now_utc())
        {
            return Err(AppError::Validation(
                "memory expires_at must be in the future".into(),
            ));
        }
        if let Some(supersedes_id) = input.supersedes_memory_id {
            let superseded = self
                .repo
                .get_agent_memory_result(supersedes_id)?
                .filter(|memory| memory.company_id == input.company_id)
                .ok_or_else(|| AppError::NotFound("superseded memory not found".into()))?;
            if !matches!(
                superseded.status.as_str(),
                AGENT_MEMORY_STATUS_ARCHIVED | AGENT_MEMORY_STATUS_SUPERSEDED
            ) {
                return Err(AppError::Conflict(
                    "archive or supersede the old memory before replacing it".into(),
                ));
            }
        }
        let existing_memories = self
            .repo
            .list_company_agent_memories_result(input.company_id)?;
        let active_owned_count = existing_memories
            .iter()
            .filter(|memory| {
                memory.owner_agent_id == input.actor_agent_id
                    && matches!(
                        memory.status.as_str(),
                        AGENT_MEMORY_STATUS_DRAFT | AGENT_MEMORY_STATUS_ACTIVE
                    )
            })
            .count();
        if active_owned_count >= 1_000 {
            return Err(AppError::Conflict(
                "Agent memory limit reached; archive stale memories before adding more".into(),
            ));
        }
        if let Some(existing) = existing_memories.iter().find(|memory| {
            memory.topic_key == topic_key
                && memory.owner_agent_id == input.actor_agent_id
                && memory.scope == scope
                && memory.project_id == input.project_id
                && memory.session_id == input.session_id
                && matches!(
                    memory.status.as_str(),
                    AGENT_MEMORY_STATUS_DRAFT | AGENT_MEMORY_STATUS_ACTIVE
                )
        }) {
            return Err(AppError::Conflict(format!(
                "memory topic already exists as {}; update that distilled memory instead",
                existing.id
            )));
        }
        let now = now_utc();
        let memory = AgentMemory {
            id: Uuid::new_v4(),
            company_id: input.company_id,
            owner_agent_id: input.actor_agent_id,
            scope: scope.clone(),
            project_id: input.project_id,
            session_id: input.session_id,
            memory_tier: classification.memory_tier.clone(),
            injection_mode: if classification.memory_tier == AGENT_MEMORY_TIER_LONG_TERM {
                AGENT_MEMORY_INJECTION_ALWAYS.into()
            } else {
                AGENT_MEMORY_INJECTION_ON_DEMAND.into()
            },
            classification_reason: classification.reason,
            estimated_ttl_days: classification.estimated_ttl_days,
            injection_cost_chars: classification.injection_cost_chars,
            visibility: match scope.as_str() {
                AGENT_MEMORY_SCOPE_CONTROL => AGENT_MEMORY_VISIBILITY_CONTROL,
                AGENT_MEMORY_SCOPE_PROJECT => AGENT_MEMORY_VISIBILITY_WORKER,
                AGENT_MEMORY_SCOPE_SESSION => session_visibility
                    .expect("session memory visibility is resolved from its bound session"),
                _ => AGENT_MEMORY_VISIBILITY_BOTH,
            }
            .into(),
            memory_type,
            topic_key,
            title,
            summary,
            when_to_use,
            tags,
            importance,
            confidence,
            pinned: false,
            status: AGENT_MEMORY_STATUS_ACTIVE.into(),
            source_refs: input.source_refs,
            supersedes_memory_id: input.supersedes_memory_id,
            expires_at: input.expires_at.or_else(|| {
                classification
                    .estimated_ttl_days
                    .map(|days| now + chrono::Duration::days(i64::from(days)))
            }),
            archived_at: None,
            verified_by_agent_id: Some(input.actor_agent_id),
            verified_by_human_user_id: None,
            verified_at: Some(now),
            created_by_agent_id: Some(input.actor_agent_id),
            created_by_human_user_id: None,
            updated_by_agent_id: Some(input.actor_agent_id),
            updated_by_human_user_id: None,
            created_at: now,
            updated_at: now,
        };
        self.repo.insert_agent_memory(memory.clone())?;
        Ok(memory)
    }

    pub fn search_agent_memories(
        &self,
        input: SearchAgentMemoriesInput,
    ) -> AppResult<Vec<AgentMemory>> {
        self.ensure_agent_can_act(input.actor_agent_id)?;
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        if membership.company_id != input.company_id {
            return Err(AppError::Unauthorized(
                "agent does not belong to the requested company".into(),
            ));
        }
        if let Some(project_id) = input.project_id {
            self.ensure_company_project_access(input.company_id, project_id, input.actor_agent_id)?;
        }
        for scope in &input.scopes {
            normalize_agent_memory_scope(scope)?;
        }
        let requested_session = input.session_id.map_or(Ok(None), |session_id| {
            self.repo
                .list_agent_codex_sessions(input.actor_agent_id, 100)
                .into_iter()
                .find(|session| session.id == session_id)
                .map(Some)
                .ok_or_else(|| {
                    AppError::Unauthorized(
                        "Agent cannot search the requested session memory".into(),
                    )
                })
        })?;
        if let (Some(project_id), Some(session)) = (input.project_id, requested_session.as_ref()) {
            if session
                .project_id
                .is_some_and(|bound_project_id| bound_project_id != project_id)
            {
                return Err(AppError::Unauthorized(
                    "Agent session is not bound to the requested project".into(),
                ));
            }
        }
        let context_project_id = input.project_id.or_else(|| {
            requested_session
                .as_ref()
                .and_then(|session| session.project_id)
        });
        let include_control_scope = context_project_id.is_none()
            && requested_session
                .as_ref()
                .is_none_or(|session| session.session_kind == AGENT_CODEX_SESSION_KIND_CONTROL);
        for memory_tier in &input.memory_tiers {
            normalize_agent_memory_tier(memory_tier)?;
        }
        for memory_type in &input.memory_types {
            normalize_agent_memory_type(memory_type)?;
        }
        let requested_status = input
            .status
            .as_deref()
            .map(normalize_agent_memory_status)
            .transpose()?;
        let query_tokens = input
            .query
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .map(|token| token.to_lowercase())
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>();
        let now = now_utc();
        self.repo
            .archive_expired_agent_memories(input.company_id, now)?;
        let mut memories = self
            .repo
            .list_company_agent_memories(input.company_id)
            .into_iter()
            .filter(|memory| memory.owner_agent_id == input.actor_agent_id)
            .filter(|memory| memory.expires_at.is_none_or(|expires_at| expires_at > now))
            .filter(|memory| {
                if !input.scopes.is_empty() && !input.scopes.contains(&memory.scope) {
                    return false;
                }
                memory.scope == AGENT_MEMORY_SCOPE_AGENT
                    || (include_control_scope && memory.scope == AGENT_MEMORY_SCOPE_CONTROL)
                    || (memory.scope == AGENT_MEMORY_SCOPE_PROJECT
                        && memory.project_id == context_project_id)
                    || (memory.scope == AGENT_MEMORY_SCOPE_SESSION
                        && memory.session_id == input.session_id)
            })
            .filter(|memory| {
                requested_status.as_ref().map_or_else(
                    || memory.status == AGENT_MEMORY_STATUS_ACTIVE,
                    |status| &memory.status == status,
                )
            })
            .filter(|memory| {
                input.memory_tiers.is_empty() || input.memory_tiers.contains(&memory.memory_tier)
            })
            .filter(|memory| {
                input.memory_types.is_empty() || input.memory_types.contains(&memory.memory_type)
            })
            .filter(|memory| {
                input.tags.is_empty()
                    || input.tags.iter().all(|tag| {
                        memory
                            .tags
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(tag))
                    })
            })
            .filter(|memory| {
                if query_tokens.is_empty() {
                    return true;
                }
                let searchable = format!(
                    "{} {} {} {} {}",
                    memory.topic_key,
                    memory.title,
                    memory.summary,
                    memory.when_to_use,
                    memory.tags.join(" ")
                )
                .to_lowercase();
                query_tokens.iter().all(|token| searchable.contains(token))
            })
            .collect::<Vec<_>>();
        memories.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.importance.cmp(&left.importance))
                .then_with(|| right.confidence.cmp(&left.confidence))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        memories.truncate(input.limit.unwrap_or(10).clamp(1, 20));
        Ok(memories)
    }

    pub fn get_agent_memory(&self, input: GetAgentMemoryInput) -> AppResult<AgentMemory> {
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        let memory = self
            .repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        if membership.company_id != input.company_id
            || memory.owner_agent_id != input.actor_agent_id
        {
            return Err(AppError::Unauthorized(
                "Agent cannot read the requested memory".into(),
            ));
        }
        Ok(memory)
    }

    pub fn update_agent_memory(&self, input: UpdateAgentMemoryInput) -> AppResult<AgentMemory> {
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        let mut memory = self
            .repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        if membership.company_id != input.company_id
            || memory.owner_agent_id != input.actor_agent_id
        {
            return Err(AppError::Unauthorized(
                "Agent cannot edit the requested memory".into(),
            ));
        }
        let requested_memory_tier = input
            .memory_tier
            .as_deref()
            .map(normalize_agent_memory_tier)
            .transpose()?
            .unwrap_or_else(|| memory.memory_tier.clone());
        if let Some(title) = input.title {
            memory.title = normalize_agent_memory_text(title, 200, "memory title", 1)?;
        }
        if let Some(summary) = input.summary {
            memory.summary = normalize_agent_memory_summary(summary)?;
        }
        if let Some(when_to_use) = input.when_to_use {
            memory.when_to_use =
                normalize_agent_memory_optional_text(when_to_use, 1_000, "memory usage context")?;
        }
        let classification = classify_agent_memory(
            &requested_memory_tier,
            &memory.memory_type,
            &memory.title,
            &memory.summary,
            &memory.when_to_use,
        );
        memory.memory_tier = classification.memory_tier;
        memory.injection_mode = if memory.memory_tier == AGENT_MEMORY_TIER_LONG_TERM {
            AGENT_MEMORY_INJECTION_ALWAYS.into()
        } else {
            AGENT_MEMORY_INJECTION_ON_DEMAND.into()
        };
        memory.classification_reason = classification.reason;
        memory.estimated_ttl_days = classification.estimated_ttl_days;
        memory.injection_cost_chars = classification.injection_cost_chars;
        if memory.memory_tier == AGENT_MEMORY_TIER_SHORT_TERM && memory.expires_at.is_none() {
            memory.expires_at = memory
                .estimated_ttl_days
                .map(|days| now_utc() + chrono::Duration::days(i64::from(days)));
        }
        if let Some(tags) = input.tags {
            memory.tags = normalize_agent_memory_tags(tags)?;
        }
        if let Some(importance) = input.importance {
            if !(1..=5).contains(&importance) {
                return Err(AppError::Validation(
                    "memory importance must be between 1 and 5".into(),
                ));
            }
            memory.importance = importance;
        }
        if let Some(confidence) = input.confidence {
            if !(0..=100).contains(&confidence) {
                return Err(AppError::Validation(
                    "memory confidence must be between 0 and 100".into(),
                ));
            }
            memory.confidence = confidence;
        }
        if input.clear_expires_at {
            memory.expires_at = None;
        } else if let Some(expires_at) = input.expires_at {
            if expires_at <= now_utc() {
                return Err(AppError::Validation(
                    "memory expires_at must be in the future".into(),
                ));
            }
            memory.expires_at = Some(expires_at);
        }
        memory.updated_by_agent_id = Some(input.actor_agent_id);
        memory.updated_by_human_user_id = None;
        memory.updated_at = now_utc();
        self.repo.update_agent_memory(memory.clone())?;
        Ok(memory)
    }

    pub fn set_agent_memory_state(
        &self,
        input: SetAgentMemoryStateInput,
    ) -> AppResult<AgentMemory> {
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        let mut memory = self
            .repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        if membership.company_id != input.company_id
            || memory.owner_agent_id != input.actor_agent_id
        {
            return Err(AppError::Unauthorized(
                "Agent cannot manage the requested memory".into(),
            ));
        }
        if let Some(status) = input.status.as_deref() {
            let status = normalize_agent_memory_status(status)?;
            if status == AGENT_MEMORY_STATUS_DRAFT {
                return Err(AppError::Validation(
                    "private memory cannot be draft".into(),
                ));
            }
            memory.status = status.clone();
            memory.archived_at = matches!(
                status.as_str(),
                AGENT_MEMORY_STATUS_ARCHIVED | AGENT_MEMORY_STATUS_SUPERSEDED
            )
            .then_some(now_utc());
            if status == AGENT_MEMORY_STATUS_ACTIVE {
                let now = now_utc();
                memory.archived_at = None;
                memory.verified_by_agent_id = Some(input.actor_agent_id);
                memory.verified_by_human_user_id = None;
                memory.verified_at = Some(now);
            }
        }
        if let Some(pinned) = input.pinned {
            if pinned {
                let pinned_count = self
                    .repo
                    .list_company_agent_memories(input.company_id)
                    .into_iter()
                    .filter(|candidate| {
                        candidate.pinned
                            && candidate.owner_agent_id == memory.owner_agent_id
                            && candidate.status == AGENT_MEMORY_STATUS_ACTIVE
                    })
                    .count();
                if pinned_count >= 20 && !memory.pinned {
                    return Err(AppError::Conflict(
                        "an Agent can pin at most 20 active memories".into(),
                    ));
                }
            }
            memory.pinned = pinned;
        }
        memory.updated_by_agent_id = Some(input.actor_agent_id);
        memory.updated_by_human_user_id = None;
        memory.updated_at = now_utc();
        self.repo.update_agent_memory(memory.clone())?;
        Ok(memory)
    }

    pub fn forget_agent_memory(&self, input: GetAgentMemoryInput) -> AppResult<()> {
        let membership = self.get_active_company_agent_membership(input.actor_agent_id)?;
        let memory = self
            .repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        let can_forget = membership.company_id == input.company_id
            && memory.owner_agent_id == input.actor_agent_id;
        if !can_forget {
            return Err(AppError::Unauthorized(
                "Agent cannot forget the requested memory".into(),
            ));
        }
        self.repo.delete_agent_memory(memory.id)
    }

    pub fn agent_memory_overview(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        project_id: Option<Uuid>,
    ) -> AppResult<AgentMemoryOverview> {
        self.agent_memory_overview_for_context(actor_agent_id, company_id, project_id, None)
    }

    fn agent_memory_overview_for_context(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        project_id: Option<Uuid>,
        session_id: Option<Uuid>,
    ) -> AppResult<AgentMemoryOverview> {
        let membership = self.get_active_company_agent_membership(actor_agent_id)?;
        if membership.company_id != company_id {
            return Err(AppError::Unauthorized(
                "agent does not belong to the requested company".into(),
            ));
        }
        let now = now_utc();
        self.repo.archive_expired_agent_memories(company_id, now)?;
        let mut visible = self
            .repo
            .list_company_agent_memories(company_id)
            .into_iter()
            .filter(|memory| memory.owner_agent_id == actor_agent_id)
            .filter(|memory| memory.expires_at.is_none_or(|expires_at| expires_at > now))
            .filter(|memory| {
                memory.scope == AGENT_MEMORY_SCOPE_AGENT
                    || (project_id.is_none() && memory.scope == AGENT_MEMORY_SCOPE_CONTROL)
                    || (memory.scope == AGENT_MEMORY_SCOPE_PROJECT
                        && memory.project_id == project_id)
                    || (memory.scope == AGENT_MEMORY_SCOPE_SESSION
                        && memory.session_id == session_id)
            })
            .collect::<Vec<_>>();
        let active_count = visible
            .iter()
            .filter(|memory| memory.status == AGENT_MEMORY_STATUS_ACTIVE)
            .count();
        let short_term_count = visible
            .iter()
            .filter(|memory| {
                memory.status == AGENT_MEMORY_STATUS_ACTIVE
                    && memory.memory_tier == AGENT_MEMORY_TIER_SHORT_TERM
            })
            .count();
        let long_term_count = visible
            .iter()
            .filter(|memory| {
                memory.status == AGENT_MEMORY_STATUS_ACTIVE
                    && memory.memory_tier == AGENT_MEMORY_TIER_LONG_TERM
            })
            .count();
        let mut long_term = visible
            .iter()
            .filter(|memory| {
                memory.status == AGENT_MEMORY_STATUS_ACTIVE
                    && memory.memory_tier == AGENT_MEMORY_TIER_LONG_TERM
            })
            .cloned()
            .collect::<Vec<_>>();
        long_term.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.importance.cmp(&left.importance))
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        let mut pinned = visible
            .iter()
            .filter(|memory| memory.status == AGENT_MEMORY_STATUS_ACTIVE && memory.pinned)
            .cloned()
            .collect::<Vec<_>>();
        pinned.sort_by(|left, right| {
            right
                .importance
                .cmp(&left.importance)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        pinned.truncate(8);
        visible.retain(|memory| memory.status == AGENT_MEMORY_STATUS_ACTIVE);
        visible.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        visible.truncate(8);
        Ok(AgentMemoryOverview {
            active_count,
            short_term_count,
            long_term_count,
            long_term,
            pinned,
            recent: visible,
        })
    }

    pub fn agent_long_term_memories(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
    ) -> AppResult<Vec<AgentMemory>> {
        Ok(self
            .agent_memory_overview(actor_agent_id, company_id, None)?
            .long_term)
    }

    pub fn agent_long_term_memories_for_control_session(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        session_id: Option<Uuid>,
    ) -> AppResult<Vec<AgentMemory>> {
        Ok(self
            .agent_memory_overview_for_context(actor_agent_id, company_id, None, session_id)?
            .long_term)
    }

    pub fn agent_long_term_memories_for_project(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
    ) -> AppResult<Vec<AgentMemory>> {
        Ok(self
            .agent_memory_overview(actor_agent_id, company_id, Some(project_id))?
            .long_term)
    }

    pub fn agent_long_term_memories_for_project_session(
        &self,
        actor_agent_id: Uuid,
        company_id: Uuid,
        project_id: Uuid,
        session_id: Option<Uuid>,
    ) -> AppResult<Vec<AgentMemory>> {
        Ok(self
            .agent_memory_overview_for_context(
                actor_agent_id,
                company_id,
                Some(project_id),
                session_id,
            )?
            .long_term)
    }

    pub fn list_company_memories_for_human(
        &self,
        input: ListCompanyMemoriesForHumanInput,
    ) -> AppResult<Vec<AgentMemory>> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let requested_memory_tier = input
            .memory_tier
            .as_deref()
            .map(normalize_agent_memory_tier)
            .transpose()?;
        let requested_status = input
            .status
            .as_deref()
            .map(normalize_agent_memory_status)
            .transpose()?;
        let requested_scope = input
            .scope
            .as_deref()
            .map(normalize_agent_memory_scope)
            .transpose()?;
        let query = input.query.unwrap_or_default().to_lowercase();
        let mut memories = self
            .repo
            .list_company_agent_memories(input.company_id)
            .into_iter()
            .filter(|memory| {
                input
                    .owner_agent_id
                    .is_none_or(|agent_id| memory.owner_agent_id == agent_id)
            })
            .filter(|memory| {
                requested_scope
                    .as_ref()
                    .is_none_or(|scope| &memory.scope == scope)
            })
            .filter(|memory| {
                input
                    .project_id
                    .is_none_or(|id| memory.project_id == Some(id))
            })
            .filter(|memory| {
                requested_memory_tier
                    .as_ref()
                    .is_none_or(|memory_tier| &memory.memory_tier == memory_tier)
            })
            .filter(|memory| {
                requested_status
                    .as_ref()
                    .is_none_or(|status| &memory.status == status)
            })
            .filter(|memory| {
                query.is_empty()
                    || format!(
                        "{} {} {} {} {}",
                        memory.topic_key,
                        memory.title,
                        memory.summary,
                        memory.when_to_use,
                        memory.tags.join(" ")
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect::<Vec<_>>();
        memories.sort_by(|left, right| {
            right
                .pinned
                .cmp(&left.pinned)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        memories.truncate(input.limit.clamp(1, 500));
        Ok(memories)
    }

    pub fn update_agent_memory_for_human(
        &self,
        input: UpdateAgentMemoryForHumanInput,
    ) -> AppResult<AgentMemory> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        let mut memory = self
            .repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        let requested_memory_tier = input
            .memory_tier
            .as_deref()
            .map(normalize_agent_memory_tier)
            .transpose()?
            .unwrap_or_else(|| memory.memory_tier.clone());
        if let Some(title) = input.title {
            memory.title = normalize_agent_memory_text(title, 200, "memory title", 1)?;
        }
        if let Some(summary) = input.summary {
            memory.summary = normalize_agent_memory_summary(summary)?;
        }
        if let Some(when_to_use) = input.when_to_use {
            memory.when_to_use =
                normalize_agent_memory_optional_text(when_to_use, 1_000, "memory usage context")?;
        }
        let classification = classify_agent_memory(
            &requested_memory_tier,
            &memory.memory_type,
            &memory.title,
            &memory.summary,
            &memory.when_to_use,
        );
        memory.memory_tier = classification.memory_tier;
        memory.injection_mode = if memory.memory_tier == AGENT_MEMORY_TIER_LONG_TERM {
            AGENT_MEMORY_INJECTION_ALWAYS.into()
        } else {
            AGENT_MEMORY_INJECTION_ON_DEMAND.into()
        };
        memory.classification_reason = classification.reason;
        memory.estimated_ttl_days = classification.estimated_ttl_days;
        memory.injection_cost_chars = classification.injection_cost_chars;
        if memory.memory_tier == AGENT_MEMORY_TIER_SHORT_TERM && memory.expires_at.is_none() {
            memory.expires_at = memory
                .estimated_ttl_days
                .map(|days| now_utc() + chrono::Duration::days(i64::from(days)));
        }
        if let Some(tags) = input.tags {
            memory.tags = normalize_agent_memory_tags(tags)?;
        }
        if let Some(importance) = input.importance {
            if !(1..=5).contains(&importance) {
                return Err(AppError::Validation(
                    "memory importance must be between 1 and 5".into(),
                ));
            }
            memory.importance = importance;
        }
        if let Some(confidence) = input.confidence {
            if !(0..=100).contains(&confidence) {
                return Err(AppError::Validation(
                    "memory confidence must be between 0 and 100".into(),
                ));
            }
            memory.confidence = confidence;
        }
        if let Some(status) = input.status {
            memory.status = normalize_agent_memory_status(&status)?;
            if memory.status == AGENT_MEMORY_STATUS_DRAFT {
                return Err(AppError::Validation(
                    "private memory cannot be draft".into(),
                ));
            }
            if memory.status == AGENT_MEMORY_STATUS_ACTIVE {
                let now = now_utc();
                memory.archived_at = None;
                memory.verified_by_agent_id = None;
                memory.verified_by_human_user_id = Some(input.human_user_id);
                memory.verified_at = Some(now);
            } else if matches!(
                memory.status.as_str(),
                AGENT_MEMORY_STATUS_ARCHIVED | AGENT_MEMORY_STATUS_SUPERSEDED
            ) {
                memory.archived_at = Some(now_utc());
            }
        }
        if let Some(pinned) = input.pinned {
            memory.pinned = pinned;
        }
        memory.updated_by_agent_id = None;
        memory.updated_by_human_user_id = Some(input.human_user_id);
        memory.updated_at = now_utc();
        self.repo.update_agent_memory(memory.clone())?;
        Ok(memory)
    }

    pub fn delete_agent_memory_for_human(
        &self,
        input: DeleteAgentMemoryForHumanInput,
    ) -> AppResult<()> {
        self.ensure_company_human_manager(input.company_id, input.human_user_id)?;
        self.repo
            .get_agent_memory(input.memory_id)
            .filter(|memory| memory.company_id == input.company_id)
            .ok_or_else(|| AppError::NotFound("Agent memory not found".into()))?;
        self.repo.delete_agent_memory(input.memory_id)
    }

    pub fn upsert_company_project_asset_refresh_for_human(
        &self,
        input: UpsertCompanyProjectAssetRefreshForHumanInput,
    ) -> AppResult<CompanyProjectAssetRefreshConfig> {
        let project = self.ensure_company_project_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.project_id,
        )?;
        self.ensure_project_not_paused(&project)?;
        self.ensure_company_agent_for_human_manager(
            input.human_user_id,
            input.company_id,
            input.maintainer_agent_id,
        )?;
        self.ensure_active_project_member(input.project_id, input.maintainer_agent_id)?;
        self.ensure_company_agent_permission(
            input.company_id,
            input.maintainer_agent_id,
            COMPANY_PERMISSION_PROJECT_ASSETS_MANAGE,
        )?;
        if !(5..=10_080).contains(&input.interval_minutes) {
            return Err(AppError::Validation(
                "asset refresh interval_minutes must be between 5 and 10080".into(),
            ));
        }
        let existing = self
            .repo
            .get_company_project_asset_refresh_config(input.project_id);
        let now = now_utc();
        let schedule_changed = existing.as_ref().is_none_or(|config| {
            config.maintainer_agent_id != input.maintainer_agent_id
                || config.interval_minutes != input.interval_minutes
                || config.enabled != input.enabled
        });
        let next_refresh_at = if input.run_now && input.enabled {
            now
        } else if !schedule_changed {
            existing
                .as_ref()
                .map_or(now, |config| config.next_refresh_at)
        } else {
            now + Duration::minutes(i64::from(input.interval_minutes))
        };
        let config = CompanyProjectAssetRefreshConfig {
            project_id: input.project_id,
            maintainer_agent_id: input.maintainer_agent_id,
            interval_minutes: input.interval_minutes,
            enabled: input.enabled,
            next_refresh_at,
            last_requested_at: existing
                .as_ref()
                .and_then(|config| config.last_requested_at),
            last_completed_at: existing
                .as_ref()
                .and_then(|config| config.last_completed_at),
            created_by_human_user_id: existing.as_ref().map_or(input.human_user_id, |config| {
                config.created_by_human_user_id
            }),
            updated_by_human_user_id: Some(input.human_user_id),
            created_at: existing.as_ref().map_or(now, |config| config.created_at),
            updated_at: now,
        };
        self.repo
            .save_company_project_asset_refresh_config(config.clone())?;
        Ok(config)
    }
}
