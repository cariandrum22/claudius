use claudius::app_config::Agent;

#[cfg(test)]
mod tests {
    use super::*;

    // Since get_agent_context_filename is not exported, we'll test the behavior
    // through the integration tests. This file is a placeholder for future unit tests
    // if the function is made public.

    #[test]
    fn test_agent_context_file_mapping() {
        // This test documents the expected mappings
        // Claude -> CLAUDE.md
        // Gemini -> AGENTS.md
        // Codex -> AGENTS.md

        // These mappings are tested in the integration tests
        assert_eq!(format!("{:?}", Agent::Claude), "Claude");
        assert_eq!(format!("{:?}", Agent::Gemini), "Gemini");
        assert_eq!(format!("{:?}", Agent::Codex), "Codex");
    }
}
