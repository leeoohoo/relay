use super::*;
use std::collections::HashSet;

impl GatePlatformRepository for MemoryPlatformRepository {
    fn insert_project_gate(&self, gate: ProjectGate) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if guard.project_gates.values().any(|existing| {
            existing.project_id == gate.project_id && existing.gate_key == gate.gate_key
        }) {
            return Err(AppError::Conflict("project gate key already exists".into()));
        }
        guard.project_gates.insert(gate.id, gate);
        Ok(())
    }

    fn get_project_gate(&self, gate_id: Uuid) -> Option<ProjectGate> {
        self.inner
            .read()
            .expect("memory repo lock poisoned")
            .project_gates
            .get(&gate_id)
            .cloned()
    }

    fn list_project_gates(&self, project_id: Uuid) -> Vec<ProjectGate> {
        let mut gates = self
            .inner
            .read()
            .expect("memory repo lock poisoned")
            .project_gates
            .values()
            .filter(|gate| gate.project_id == project_id)
            .cloned()
            .collect::<Vec<_>>();
        gates.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        gates
    }

    fn update_project_gate(&self, gate: ProjectGate) -> AppResult<()> {
        let mut guard = self.inner.write().expect("memory repo lock poisoned");
        if !guard.project_gates.contains_key(&gate.id) {
            return Err(AppError::NotFound("project gate not found".into()));
        }
        guard.project_gates.insert(gate.id, gate);
        Ok(())
    }

    fn save_project_task_gate_requirement(
        &self,
        requirement: ProjectTaskGateRequirement,
    ) -> AppResult<()> {
        self.inner
            .write()
            .expect("memory repo lock poisoned")
            .project_task_gate_requirements
            .insert((requirement.task_id, requirement.gate_id), requirement);
        Ok(())
    }

    fn list_project_task_gate_requirements(
        &self,
        project_id: Uuid,
    ) -> Vec<ProjectTaskGateRequirement> {
        let guard = self.inner.read().expect("memory repo lock poisoned");
        let gate_ids = guard
            .project_gates
            .values()
            .filter(|gate| gate.project_id == project_id)
            .map(|gate| gate.id)
            .collect::<HashSet<_>>();
        guard
            .project_task_gate_requirements
            .values()
            .filter(|requirement| gate_ids.contains(&requirement.gate_id))
            .cloned()
            .collect()
    }
}
