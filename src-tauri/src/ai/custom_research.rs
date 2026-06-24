pub(crate) fn local_context_content(query: &str, contexts: &[String]) -> String {
    if contexts.is_empty() {
        return format!("## Research: {}\n\nNo local matches found.", query);
    }

    let mut buf = format!("## Research: {}\n\n### From your notes:\n\n", query);
    for ctx in contexts {
        buf.push_str(&format!(
            "- {}\n",
            ctx.chars().take(200).collect::<String>()
        ));
    }
    buf
}

pub(crate) fn no_llm_content(query: &str, contexts: &[String]) -> String {
    let mut buf = local_context_content(query, contexts);
    if contexts.is_empty() {
        buf.push_str(" Add an LLM API key for external search.");
    } else {
        buf.push_str("\n\n*Add an LLM API key to enable AI-powered synthesis.*");
    }
    buf
}

pub(crate) fn llm_user_prompt(
    query: &str,
    contexts: &[String],
    expected_output: Option<&str>,
) -> String {
    let context_text = if contexts.is_empty() {
        "No local note context matched this request.".to_string()
    } else {
        contexts
            .iter()
            .enumerate()
            .map(|(index, context)| format!("[Context {}]\n{}", index + 1, context))
            .collect::<Vec<_>>()
            .join("\n\n")
    };
    let expected_output = expected_output
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Produce a concise research brief in Markdown.");

    format!(
        "Task:\n{}\n\nExpected output:\n{}\n\nLocal note context:\n{}\n\nUse the available context when relevant. If no local context matched, answer from general knowledge and state that no local note match was found.",
        query, expected_output, context_text
    )
}

pub(crate) fn llm_content(query: &str, response: &str, model: &str) -> String {
    format!(
        "## Research: {}\n\n{}\n\n---\n\nLLM model: {}",
        query,
        response.trim(),
        model
    )
}
