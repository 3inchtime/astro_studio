CREATE TABLE IF NOT EXISTS prompt_agent_sessions (
    id TEXT PRIMARY KEY,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'active',
    original_prompt TEXT NOT NULL,
    draft_prompt TEXT,
    accepted_prompt TEXT,
    selected_skill_ids TEXT NOT NULL DEFAULT '[]',
    suggested_params TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE TABLE IF NOT EXISTS prompt_agent_messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES prompt_agent_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    draft_prompt TEXT,
    selected_skill_ids TEXT NOT NULL DEFAULT '[]',
    suggested_params TEXT NOT NULL DEFAULT '{}',
    ready_to_generate INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_conversation_id ON prompt_agent_sessions(conversation_id);
CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_project_id ON prompt_agent_sessions(project_id);
CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_updated_at ON prompt_agent_sessions(updated_at);
CREATE INDEX IF NOT EXISTS idx_prompt_agent_messages_session_id ON prompt_agent_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_prompt_agent_messages_created_at ON prompt_agent_messages(created_at);
