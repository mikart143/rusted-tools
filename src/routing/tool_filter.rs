use crate::config::ToolFilter;
use crate::mcp::ToolDefinition;

impl ToolFilter {
    /// Check if a tool should be allowed based on include/exclude filters
    /// Include list takes precedence - if present, tool must be in it
    /// Exclude list is then checked - if present, tool must not be in it
    pub(crate) fn allows(&self, tool_name: &str) -> bool {
        // If include list exists, tool must be in it
        if let Some(include) = &self.include
            && !include.iter().any(|t| t == tool_name)
        {
            return false;
        }

        // If exclude list exists, tool must not be in it
        if let Some(exclude) = &self.exclude
            && exclude.iter().any(|t| t == tool_name)
        {
            return false;
        }

        true
    }
}

/// Apply tool filters to a list of tools
pub(crate) fn apply_tool_filter(
    tools: Vec<ToolDefinition>,
    filter: Option<&ToolFilter>,
) -> Vec<ToolDefinition> {
    match filter {
        None => tools, // No filter, return all tools
        Some(filter) => tools
            .into_iter()
            .filter(|tool| filter.allows(&tool.name))
            .collect(),
    }
}

/// Check if a specific tool name is allowed by the filter
pub(crate) fn is_tool_allowed(tool_name: &str, filter: Option<&ToolFilter>) -> bool {
    match filter {
        None => true, // No filter, all tools allowed
        Some(filter) => filter.allows(tool_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: Some(format!("Test tool {}", name)),
            input_schema: json!({}),
        }
    }

    #[test]
    fn test_apply_no_filter() {
        let tools = vec![
            create_test_tool("tool1"),
            create_test_tool("tool2"),
            create_test_tool("tool3"),
        ];

        let filtered = apply_tool_filter(tools.clone(), None);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_apply_include_filter() {
        let tools = vec![
            create_test_tool("tool1"),
            create_test_tool("tool2"),
            create_test_tool("tool3"),
        ];

        let filter = ToolFilter {
            include: Some(vec!["tool1".to_string(), "tool2".to_string()]),
            exclude: None,
        };

        let filtered = apply_tool_filter(tools, Some(&filter));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "tool1");
        assert_eq!(filtered[1].name, "tool2");
    }

    #[test]
    fn test_apply_exclude_filter() {
        let tools = vec![
            create_test_tool("tool1"),
            create_test_tool("tool2"),
            create_test_tool("tool3"),
        ];

        let filter = ToolFilter {
            include: None,
            exclude: Some(vec!["tool2".to_string()]),
        };

        let filtered = apply_tool_filter(tools, Some(&filter));
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "tool1");
        assert_eq!(filtered[1].name, "tool3");
    }

    #[test]
    fn test_is_tool_allowed_no_filter() {
        assert!(is_tool_allowed("any_tool", None));
    }

    #[test]
    fn test_is_tool_allowed_with_include() {
        let filter = ToolFilter {
            include: Some(vec!["allowed_tool".to_string()]),
            exclude: None,
        };

        assert!(is_tool_allowed("allowed_tool", Some(&filter)));
        assert!(!is_tool_allowed("other_tool", Some(&filter)));
    }

    #[test]
    fn test_is_tool_allowed_with_exclude() {
        let filter = ToolFilter {
            include: None,
            exclude: Some(vec!["blocked_tool".to_string()]),
        };

        assert!(!is_tool_allowed("blocked_tool", Some(&filter)));
        assert!(is_tool_allowed("other_tool", Some(&filter)));
    }
}
