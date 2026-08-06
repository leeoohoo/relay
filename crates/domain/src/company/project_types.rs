mod catalog;
mod inference;
mod rules;

pub use catalog::company_project_type_catalog;
pub use inference::{company_project_type_by_key, infer_company_project_type};
