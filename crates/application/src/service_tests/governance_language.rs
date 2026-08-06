use super::*;

#[test]
fn governance_skill_language_defaults_to_chinese_and_accepts_english() {
    let defaults = default_company_governance_policy_settings();
    assert_eq!(
        defaults.skill_language,
        ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_ZH_CN
    );
    validate_company_governance_policy_settings(&defaults)
        .expect("the default governance language should be valid");

    let mut english = defaults.clone();
    english.skill_language = ai_chat_domain::company::COMPANY_SKILL_LANGUAGE_EN.into();
    validate_company_governance_policy_settings(&english)
        .expect("English should be a supported Skill language");

    english.skill_language = "fr".into();
    assert!(matches!(
        validate_company_governance_policy_settings(&english),
        Err(AppError::Validation(message)) if message.contains("zh-CN or en")
    ));
}
